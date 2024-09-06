use bevy::color::palettes;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};

const MAP_SIDE_LENGTH_X: u32 = 74;
const MAP_SIDE_LENGTH_Y: u32 = 46;

const TILE_SIZE_HEX_ROW: TilemapTileSize = TilemapTileSize { x: 100.0, y: 115.0 };
const GRID_SIZE_HEX_ROW: TilemapGridSize = TilemapGridSize { x: 100.0, y: 115.0 };

#[derive(Deref, Resource)]
struct FontHandle(Handle<Font>);

#[derive(Resource)]
struct CursorPos(Vec2);

#[derive(Component)]
struct TileLabel(Entity);

#[derive(Component)]
struct HighlightedLabel;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
struct SpawnTilemapSet;

impl FromWorld for FontHandle {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self(asset_server.load("fonts/NotoSans/NotoSans-Regular.ttf"))
    }
}

impl Default for CursorPos {
    fn default() -> Self {
        // Initialize the cursor pos at some far away place. It will get updated
        // correctly when the cursor moves.
        Self(Vec2::new(-1000.0, -1000.0))
    }
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Hexciv".to_owned(),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(PanCamPlugin)
        .add_plugins(TilemapPlugin)
        .init_resource::<FontHandle>()
        .init_resource::<CursorPos>()
        .add_systems(
            Startup,
            (spawn_tilemap, apply_deferred)
                .chain()
                .in_set(SpawnTilemapSet),
        )
        .add_systems(Startup, spawn_tile_labels.after(SpawnTilemapSet))
        .add_systems(Update, (update_cursor_pos, highlight_tile_labels).chain())
        .run();
}

/// Generates the initial tilemap.
fn spawn_tilemap(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default()).insert(PanCam {
        grab_buttons: vec![MouseButton::Left],
        min_scale: 1.0,
        max_scale: 10.0,
        ..Default::default()
    });

    let image_handles = vec![
        asset_server.load("tiles/plains.png"),
        asset_server.load("tiles/grassland.png"),
        asset_server.load("tiles/desert.png"),
        asset_server.load("tiles/tundra.png"),
        asset_server.load("tiles/snow.png"),
        asset_server.load("tiles/mountains.png"),
        asset_server.load("tiles/coast.png"),
        asset_server.load("tiles/ocean.png"),
    ];
    let texture_vec = TilemapTexture::Vector(image_handles);

    let map_size = TilemapSize {
        x: MAP_SIDE_LENGTH_X,
        y: MAP_SIDE_LENGTH_Y,
    };

    let mut tile_storage = TileStorage::empty(map_size);
    let tilemap_entity = commands.spawn_empty().id();
    let tilemap_id = TilemapId(tilemap_entity);

    let mut rng = fastrand::Rng::new();
    let tile_choices = [
        TileTextureIndex(0),
        TileTextureIndex(1),
        TileTextureIndex(2),
        TileTextureIndex(3),
        TileTextureIndex(4),
        TileTextureIndex(5),
        TileTextureIndex(6),
        TileTextureIndex(7),
    ];
    commands.entity(tilemap_id.0).with_children(|parent| {
        for x in 0..map_size.x {
            for y in 0..map_size.y {
                let tile_pos = TilePos { x, y };
                let tile_entity = parent
                    .spawn(TileBundle {
                        position: tile_pos,
                        tilemap_id,
                        texture_index: rng.choice(tile_choices).unwrap(),
                        ..Default::default()
                    })
                    .id();
                tile_storage.set(&tile_pos, tile_entity);
            }
        }
    });

    let tile_size = TILE_SIZE_HEX_ROW;
    let grid_size = GRID_SIZE_HEX_ROW;
    let map_type = TilemapType::Hexagon(HexCoordSystem::RowOdd);

    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size,
        size: map_size,
        storage: tile_storage,
        texture: texture_vec,
        tile_size,
        map_type,
        transform: get_tilemap_center_transform(&map_size, &grid_size, &map_type, 0.0),
        ..Default::default()
    });
}

/// Generates tile position labels of the form: `(tile_pos.x, tile_pos.y)`
fn spawn_tile_labels(
    mut commands: Commands,
    tilemap_q: Query<(&Transform, &TilemapType, &TilemapGridSize, &TileStorage)>,
    tile_q: Query<&mut TilePos>,
    font_handle: Res<FontHandle>,
) {
    let text_style = TextStyle {
        font: font_handle.clone(),
        font_size: 20.0,
        color: Color::BLACK,
    };
    let text_justify = JustifyText::Center;
    for (map_transform, map_type, grid_size, tilemap_storage) in tilemap_q.iter() {
        for tile_entity in tilemap_storage.iter().flatten() {
            let tile_pos = tile_q.get(*tile_entity).unwrap();
            let tile_center = tile_pos.center_in_world(grid_size, map_type).extend(1.0);
            let transform = *map_transform * Transform::from_translation(tile_center);

            let label_entity = commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        format!("{x}, {y}", x = tile_pos.x, y = tile_pos.y),
                        text_style.clone(),
                    )
                    .with_justify(text_justify),
                    transform,
                    ..default()
                })
                .id();
            commands
                .entity(*tile_entity)
                .insert(TileLabel(label_entity));
        }
    }
}

/// Keeps the cursor position updated based on any `CursorMoved` events.
fn update_cursor_pos(
    camera_q: Query<(&GlobalTransform, &Camera)>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    for cursor_moved in cursor_moved_events.read() {
        // To get the mouse's world position, we have to transform its window position
        // by any transforms on the camera. This is done by projecting the
        // cursor position into camera space (world space).
        for (cam_t, cam) in camera_q.iter() {
            if let Some(pos) = cam.viewport_to_world_2d(cam_t, cursor_moved.position) {
                *cursor_pos = CursorPos(pos);
            }
        }
    }
}

/// Checks which tile the cursor is hovered over.
fn highlight_tile_labels(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    tilemap_q: Query<(
        &TilemapSize,
        &TilemapGridSize,
        &TilemapType,
        &TileStorage,
        &Transform,
    )>,
    highlighted_tiles_q: Query<Entity, With<HighlightedLabel>>,
    tile_label_q: Query<&TileLabel>,
    mut text_q: Query<&mut Text>,
) {
    // Un-highlight any previously highlighted tile labels.
    for highlighted_tile_entity in highlighted_tiles_q.iter() {
        if let Ok(label) = tile_label_q.get(highlighted_tile_entity) {
            if let Ok(mut tile_text) = text_q.get_mut(label.0) {
                for section in tile_text.sections.iter_mut() {
                    section.style.color = Color::BLACK;
                }
                commands
                    .entity(highlighted_tile_entity)
                    .remove::<HighlightedLabel>();
            }
        }
    }

    for (map_size, grid_size, map_type, tile_storage, map_transform) in tilemap_q.iter() {
        // Grab the cursor position from the `Res<CursorPos>`
        let cursor_pos: Vec2 = cursor_pos.0;
        // We need to make sure that the cursor's world position is correct relative to
        // the map due to any map transformation.
        let cursor_in_map_pos: Vec2 = {
            // Extend the cursor_pos vec3 by 0.0 and 1.0
            let cursor_pos = Vec4::from((cursor_pos, 0.0, 1.0));
            let cursor_in_map_pos = map_transform.compute_matrix().inverse() * cursor_pos;
            cursor_in_map_pos.xy()
        };
        // Once we have a world position we can transform it into a possible tile
        // position.
        if let Some(tile_pos) =
            TilePos::from_world_pos(&cursor_in_map_pos, map_size, grid_size, map_type)
        {
            // Highlight the relevant tile's label
            if let Some(tile_entity) = tile_storage.get(&tile_pos) {
                if let Ok(label) = tile_label_q.get(tile_entity) {
                    if let Ok(mut tile_text) = text_q.get_mut(label.0) {
                        for section in tile_text.sections.iter_mut() {
                            section.style.color = palettes::tailwind::RED_600.into();
                        }
                        commands.entity(tile_entity).insert(HighlightedLabel);
                    }
                }
            }
        }
    }
}
