use bevy::color::palettes;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use fastlem_random_terrain::generate_terrain;
use fastrand_contrib::RngExt as _;
use helpers::hex_grid::neighbors::HexNeighbors;

// IMPORTANT: The map's dimensions must both be even numbers, due to the
// assumptions being made in our calculations.
const MAP_SIDE_LENGTH_X: u32 = 74;
const MAP_SIDE_LENGTH_Y: u32 = 46;

// IMPORTANT: The tile's dimensions must follow the aspect ratio of a regular
// hexagon.
const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 100.0, y: 115.0 };
const GRID_SIZE: TilemapGridSize = TilemapGridSize {
    x: 100.0,
    y: 115.47005,
};

const ODD_ROW_OFFSET: f64 = 0.5 * GRID_SIZE.x as f64;

const TILE_CENTER_TO_CENTER_X: f64 = GRID_SIZE.x as f64;
const TILE_CENTER_TO_CENTER_Y: f64 = 0.8660254 * GRID_SIZE.x as f64;

const TILE_LABEL_Z_INDEX: f32 = 2.0;

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
enum BaseTerrain {
    Plains = 0,
    Grassland = 1,
    Desert = 2,
    Tundra = 3,
    Snow = 4,
    Coast = 15,
    Ocean = 16,
}

#[derive(Copy, Clone)]
#[repr(u32)]
enum BaseTerrainVariant {
    Hills = 5,
    Mountains = 10,
}

#[derive(Copy, Clone)]
#[repr(u32)]
enum TerrainFeatures {
    // TODO: woods
    // TODO: rainforest
    // TODO: marsh
    // TODO: floodplains
    Oasis = 0,
    // TODO: reef
    Ice = 1,
}

enum EarthLatitude {
    ArticCirle,
    TropicOfCancer,
    TropicOfCapricorn,
    AntarcticCircle,
}

#[derive(Deref, Resource)]
struct FontHandle(Handle<Font>);

#[derive(Resource)]
struct MapSeed(u64);

#[derive(Resource)]
struct CursorPos(Vec2);

/// Marker for the base terrain layer tilemap.
#[derive(Component)]
struct BaseTerrainLayer;

/// Marker for the terrain features layer tilemap.
#[derive(Component)]
struct TerrainFeaturesLayer;

#[derive(Component)]
struct TileLabel(Entity);

#[derive(Component)]
struct HighlightedLabel;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
struct SpawnTilemapSet;

impl EarthLatitude {
    pub const fn latitude(&self) -> f64 {
        match self {
            EarthLatitude::ArticCirle => 66.57,
            EarthLatitude::TropicOfCancer => 23.43,
            EarthLatitude::TropicOfCapricorn => -23.43,
            EarthLatitude::AntarcticCircle => -66.57,
        }
    }
}

impl FromWorld for FontHandle {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self(asset_server.load("fonts/NotoSans/NotoSans-Regular.ttf"))
    }
}

impl FromWorld for MapSeed {
    fn from_world(_world: &mut World) -> Self {
        let rng = fastrand::Rng::new();
        Self(rng.get_seed())
    }
}

impl Default for CursorPos {
    fn default() -> Self {
        // Initialize the cursor pos at some far away place. It will get updated
        // correctly when the cursor moves.
        Self(Vec2::new(-1000.0, -1000.0))
    }
}

impl TerrainFeaturesLayer {
    const Z_INDEX: f32 = 1.0;
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
        .init_resource::<MapSeed>()
        .init_resource::<CursorPos>()
        .add_systems(
            Startup,
            (spawn_tilemap, post_spawn_tilemap)
                .chain()
                .in_set(SpawnTilemapSet),
        )
        .add_systems(Startup, spawn_tile_labels.after(SpawnTilemapSet))
        .add_systems(Update, (update_cursor_pos, highlight_tile_labels).chain())
        .run();
}

/// Generates the initial tilemap.
fn spawn_tilemap(mut commands: Commands, asset_server: Res<AssetServer>, map_seed: Res<MapSeed>) {
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
        asset_server.load("tiles/plains-hills.png"),
        asset_server.load("tiles/grassland-hills.png"),
        asset_server.load("tiles/desert-hills.png"),
        asset_server.load("tiles/tundra-hills.png"),
        asset_server.load("tiles/snow-hills.png"),
        asset_server.load("tiles/plains-mountains.png"),
        asset_server.load("tiles/grassland-mountains.png"),
        asset_server.load("tiles/desert-mountains.png"),
        asset_server.load("tiles/tundra-mountains.png"),
        asset_server.load("tiles/snow-mountains.png"),
        asset_server.load("tiles/coast.png"),
        asset_server.load("tiles/ocean.png"),
    ];
    let texture_vec = TilemapTexture::Vector(image_handles);

    let map_size = TilemapSize {
        x: MAP_SIDE_LENGTH_X,
        y: MAP_SIDE_LENGTH_Y,
    };

    const BOUND_WIDTH: f64 = ((MAP_SIDE_LENGTH_X - 1) as f64 * TILE_CENTER_TO_CENTER_X
        + GRID_SIZE.x as f64
        + ODD_ROW_OFFSET)
        / 100.0;
    const BOUND_HEIGHT: f64 =
        ((MAP_SIDE_LENGTH_Y - 1) as f64 * TILE_CENTER_TO_CENTER_Y + GRID_SIZE.y as f64) / 100.0;

    let bound_min = fastlem_random_terrain::Site2D {
        x: -BOUND_WIDTH / 2.0,
        y: -BOUND_HEIGHT / 2.0,
    };
    let bound_max = fastlem_random_terrain::Site2D {
        x: BOUND_WIDTH / 2.0,
        y: BOUND_HEIGHT / 2.0,
    };
    let bound_range = fastlem_random_terrain::Site2D {
        x: BOUND_WIDTH,
        y: BOUND_HEIGHT,
    };

    let mut rng = fastrand::Rng::new();
    rng.seed(map_seed.0);
    info!(seed = map_seed.0, "map seed");

    let terrain = {
        let config = fastlem_random_terrain::Config {
            seed: rng.u32(..),
            land_ratio: rng.f64_range(0.29..=0.6),
            ..Default::default()
        };
        info!(?config, "fastlem-random-terrain config");
        generate_terrain(&config, bound_min, bound_max, bound_range)
    };

    let mut tile_storage = TileStorage::empty(map_size);
    let tilemap_entity = commands.spawn_empty().id();
    let tilemap_id = TilemapId(tilemap_entity);

    let frigid_zone_tile_choices = vec![BaseTerrain::Tundra, BaseTerrain::Snow];
    let temperate_zone_tile_choices = vec![
        BaseTerrain::Plains,
        BaseTerrain::Plains,
        BaseTerrain::Plains,
        BaseTerrain::Grassland,
    ];
    let subtropics_tile_choices = vec![
        BaseTerrain::Plains,
        BaseTerrain::Plains,
        BaseTerrain::Plains,
        BaseTerrain::Grassland,
        BaseTerrain::Grassland,
        BaseTerrain::Desert,
        BaseTerrain::Desert,
    ];
    let tropics_tile_choices = vec![
        BaseTerrain::Plains,
        BaseTerrain::Plains,
        BaseTerrain::Grassland,
        BaseTerrain::Grassland,
        BaseTerrain::Grassland,
        BaseTerrain::Grassland,
        BaseTerrain::Desert,
    ];

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let elevation = {
                let x = bound_min.x
                    + (f64::from(GRID_SIZE.x) / 2.0
                        + f64::from(tile_pos.x) * TILE_CENTER_TO_CENTER_X
                        + if tile_pos.y % 2 == 0 {
                            0.0
                        } else {
                            ODD_ROW_OFFSET
                        })
                        / 100.0;
                let y = bound_min.y
                    + (f64::from(GRID_SIZE.y) / 2.0
                        + f64::from(map_size.y - tile_pos.y - 1) * TILE_CENTER_TO_CENTER_Y)
                        / 100.0;
                let site = fastlem_random_terrain::Site2D { x, y };
                terrain.get_elevation(&site).unwrap()
            };
            let texture_index = if elevation < 0.05 {
                TileTextureIndex(BaseTerrain::Ocean as u32)
            } else {
                let latitude =
                    -90.0 + 180.0 * ((f64::from(tile_pos.y) + 0.5) / f64::from(map_size.y));

                let choice = *rng
                    .choice(
                        if latitude >= EarthLatitude::ArticCirle.latitude()
                            || latitude <= EarthLatitude::AntarcticCircle.latitude()
                        {
                            &frigid_zone_tile_choices
                        } else if latitude >= 35.0 || latitude <= -35.0 {
                            &temperate_zone_tile_choices
                        } else if latitude >= EarthLatitude::TropicOfCancer.latitude()
                            || latitude <= EarthLatitude::TropicOfCapricorn.latitude()
                        {
                            &subtropics_tile_choices
                        } else {
                            &tropics_tile_choices
                        },
                    )
                    .unwrap();

                TileTextureIndex(if elevation >= 25.0 {
                    choice as u32 + BaseTerrainVariant::Mountains as u32
                } else if elevation >= 5.0 {
                    choice as u32 + BaseTerrainVariant::Hills as u32
                } else {
                    choice as u32
                })
            };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    tilemap_id,
                    texture_index,
                    ..Default::default()
                })
                .id();
            tile_storage.set(&tile_pos, tile_entity);
        }
    }

    let map_type = TilemapType::Hexagon(HexCoordSystem::RowOdd);

    commands
        .entity(tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: tile_storage,
            texture: texture_vec,
            tile_size: TILE_SIZE,
            map_type,
            transform: get_tilemap_center_transform(&map_size, &GRID_SIZE, &map_type, 0.0),
            ..Default::default()
        })
        .insert(BaseTerrainLayer);

    let image_handles = vec![
        // TODO: woods
        // TODO: rainforest
        // TODO: marsh
        // TODO: floodplains
        asset_server.load("tiles/oasis.png"),
        // TODO: reef
        asset_server.load("tiles/ice.png"),
    ];
    let texture_vec = TilemapTexture::Vector(image_handles);

    let tile_storage = TileStorage::empty(map_size);
    let tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: tile_storage,
            texture: texture_vec,
            tile_size: TILE_SIZE,
            map_type,
            transform: get_tilemap_center_transform(&map_size, &GRID_SIZE, &map_type, 0.0)
                * Transform::from_xyz(0.0, 0.0, TerrainFeaturesLayer::Z_INDEX),
            ..Default::default()
        })
        .insert(TerrainFeaturesLayer);
}

#[allow(clippy::type_complexity)]
fn post_spawn_tilemap(
    mut commands: Commands,
    map_seed: Res<MapSeed>,
    base_terrain_tilemap_query: Query<(&TilemapSize, &TileStorage), With<BaseTerrainLayer>>,
    mut terrain_features_tilemap_query: Query<
        (Entity, &mut TileStorage),
        (With<TerrainFeaturesLayer>, Without<BaseTerrainLayer>),
    >,
    mut tile_query: Query<&mut TileTextureIndex>,
) {
    let mut rng = fastrand::Rng::new();
    rng.seed(map_seed.0);

    let oasis_choices = [true, false, false, false, false];
    let ice_choices = [true, true, true, false];

    let (map_size, base_terrain_tile_storage) = base_terrain_tilemap_query.get_single().unwrap();
    let (terrain_features_tilemap_entity, mut terrain_features_tile_storage) =
        terrain_features_tilemap_query.get_single_mut().unwrap();
    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let tile_entity = base_terrain_tile_storage.get(&tile_pos).unwrap();
            let tile_texture = *tile_query.get(tile_entity).unwrap();

            if tile_texture.0 == BaseTerrain::Ocean as u32 {
                let neighbor_entities =
                    HexNeighbors::get_neighboring_positions_row_odd(&tile_pos, map_size)
                        .entities(base_terrain_tile_storage);
                if neighbor_entities.iter().any(|neighbor_entity| {
                    let tile_texture = tile_query.get(*neighbor_entity).unwrap();
                    tile_texture.0 != BaseTerrain::Ocean as u32
                        && tile_texture.0 != BaseTerrain::Coast as u32
                }) {
                    let mut tile_texture = tile_query.get_mut(tile_entity).unwrap();
                    tile_texture.0 = BaseTerrain::Coast as u32;
                }
            }

            if tile_texture.0 == BaseTerrain::Desert as u32 && rng.choice(oasis_choices).unwrap() {
                let tile_entity = commands
                    .spawn(TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(terrain_features_tilemap_entity),
                        texture_index: TileTextureIndex(TerrainFeatures::Oasis as u32),
                        ..Default::default()
                    })
                    .id();
                terrain_features_tile_storage.set(&tile_pos, tile_entity);
            }

            if tile_texture.0 == BaseTerrain::Ocean as u32
                || tile_texture.0 == BaseTerrain::Coast as u32
            {
                let latitude =
                    -90.0 + 180.0 * ((f64::from(tile_pos.y) + 0.5) / f64::from(map_size.y));

                if (latitude >= EarthLatitude::ArticCirle.latitude()
                    || latitude <= EarthLatitude::AntarcticCircle.latitude())
                    && rng.choice(ice_choices).unwrap()
                {
                    let tile_entity = commands
                        .spawn(TileBundle {
                            position: tile_pos,
                            tilemap_id: TilemapId(terrain_features_tilemap_entity),
                            texture_index: TileTextureIndex(TerrainFeatures::Ice as u32),
                            ..Default::default()
                        })
                        .id();
                    terrain_features_tile_storage.set(&tile_pos, tile_entity);
                }
            }
        }
    }
}

/// Generates tile position labels.
fn spawn_tile_labels(
    mut commands: Commands,
    tilemap_query: Query<
        (&Transform, &TilemapType, &TilemapGridSize, &TileStorage),
        With<BaseTerrainLayer>,
    >,
    tile_query: Query<&mut TilePos>,
    font_handle: Res<FontHandle>,
) {
    let text_style = TextStyle {
        font: font_handle.clone(),
        font_size: 20.0,
        color: Color::BLACK,
    };
    let text_justify = JustifyText::Center;
    let (map_transform, map_type, grid_size, tilemap_storage) = tilemap_query.get_single().unwrap();
    for tile_entity in tilemap_storage.iter().flatten() {
        let tile_pos = tile_query.get(*tile_entity).unwrap();
        let tile_center = tile_pos
            .center_in_world(grid_size, map_type)
            .extend(TILE_LABEL_Z_INDEX);
        let transform = *map_transform * Transform::from_translation(tile_center);

        let label_entity = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    format!("{x}, {y}", x = tile_pos.x, y = tile_pos.y),
                    text_style.clone(),
                )
                .with_justify(text_justify),
                transform,
                visibility: Visibility::Hidden,
                ..Default::default()
            })
            .id();
        commands
            .entity(*tile_entity)
            .insert(TileLabel(label_entity));
    }
}

/// Keeps the cursor position updated based on any `CursorMoved` events.
fn update_cursor_pos(
    camera_query: Query<(&GlobalTransform, &Camera)>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    for cursor_moved in cursor_moved_events.read() {
        // To get the mouse's world position, we have to transform its window position
        // by any transforms on the camera. This is done by projecting the
        // cursor position into camera space (world space).
        for (camera_transform, camera) in camera_query.iter() {
            if let Some(pos) = camera.viewport_to_world_2d(camera_transform, cursor_moved.position)
            {
                *cursor_pos = CursorPos(pos);
            }
        }
    }
}

/// Checks which tile the cursor is hovered over.
fn highlight_tile_labels(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    tilemap_query: Query<
        (
            &TilemapSize,
            &TilemapGridSize,
            &TilemapType,
            &TileStorage,
            &Transform,
        ),
        With<BaseTerrainLayer>,
    >,
    highlighted_tiles_query: Query<Entity, With<HighlightedLabel>>,
    tile_label_query: Query<&TileLabel>,
    mut text_query: Query<(&mut Text, &mut Visibility)>,
) {
    // Un-highlight any previously highlighted tile labels.
    for highlighted_tile_entity in highlighted_tiles_query.iter() {
        if let Ok(label) = tile_label_query.get(highlighted_tile_entity) {
            if let Ok((mut tile_text, mut visibility)) = text_query.get_mut(label.0) {
                for section in tile_text.sections.iter_mut() {
                    section.style.color = Color::BLACK;
                }
                *visibility = Visibility::Hidden;
                commands
                    .entity(highlighted_tile_entity)
                    .remove::<HighlightedLabel>();
            }
        }
    }

    let (map_size, grid_size, map_type, tile_storage, map_transform) =
        tilemap_query.get_single().unwrap();
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
            if let Ok(label) = tile_label_query.get(tile_entity) {
                if let Ok((mut tile_text, mut visibility)) = text_query.get_mut(label.0) {
                    for section in tile_text.sections.iter_mut() {
                        section.style.color = palettes::tailwind::RED_600.into();
                    }
                    *visibility = Visibility::Visible;
                    commands.entity(tile_entity).insert(HighlightedLabel);
                }
            }
        }
    }
}
