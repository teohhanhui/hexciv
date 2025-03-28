use bevy::color::palettes;
use bevy::ecs::system::{RunSystemOnce as _, SystemState};
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use leafwing_input_manager::common_conditions::action_toggle_active;
use leafwing_input_manager::prelude::*;

use crate::action::DebugAction;
use crate::asset::FontHandle;
use crate::game_setup::InGameSet;
use crate::input::{update_cursor_tile_pos, CursorTilePos};
use crate::layer::BaseTerrainLayerFilter;

const TILE_LABEL_Z_INDEX: f32 = 3.0;

#[derive(Component)]
struct TileLabel(Entity);

#[derive(Component)]
struct HighlightedLabel;

pub struct TileLabelPlugin;

impl Plugin for TileLabelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<DebugAction>::default())
            .init_resource::<ActionState<DebugAction>>()
            .insert_resource(DebugAction::input_map())
            .add_systems(
                Update,
                (
                    (
                        show_tile_labels
                            .run_if(action_toggle_active(false, DebugAction::ShowTileLabels)),
                        hide_tile_labels
                            .run_if(action_toggle_active(true, DebugAction::ShowTileLabels)),
                    ),
                    highlight_tile_labels.after(update_cursor_tile_pos),
                )
                    .chain()
                    .in_set(InGameSet),
            );
    }
}

/// Generates tile position labels.
fn spawn_tile_labels(
    mut commands: Commands,
    base_terrain_tilemap_query: Query<
        (&Transform, &TilemapType, &TilemapGridSize, &TileStorage),
        BaseTerrainLayerFilter,
    >,
    base_terrain_tile_query: Query<(&mut TilePos,), BaseTerrainLayerFilter>,
    font_handle: Res<FontHandle>,
) {
    let text_font = TextFont {
        font: font_handle.0.clone(),
        font_size: 20.0,
        ..Default::default()
    };
    let text_color = TextColor(Color::BLACK);
    let text_layout = TextLayout {
        justify: JustifyText::Center,
        ..Default::default()
    };
    let (map_transform, map_type, grid_size, tilemap_storage) =
        base_terrain_tilemap_query.get_single().unwrap();
    for tile_entity in tilemap_storage.iter().flatten() {
        let (tile_pos,) = base_terrain_tile_query.get(*tile_entity).unwrap();
        let tile_center = tile_pos
            .center_in_world(grid_size, map_type)
            .extend(TILE_LABEL_Z_INDEX);
        let transform = *map_transform * Transform::from_translation(tile_center);

        let label_entity = commands
            .spawn((
                Text2d::new(format!("{x}, {y}", x = tile_pos.x, y = tile_pos.y)),
                text_font.clone(),
                text_color,
                text_layout,
                transform,
            ))
            .id();
        commands
            .entity(*tile_entity)
            .insert(TileLabel(label_entity));
    }
}

#[allow(clippy::type_complexity)]
fn show_tile_labels(
    world: &mut World,
    tile_label_query: &mut QueryState<(), With<TileLabel>>,
    system_state: &mut SystemState<(Query<(&TileLabel,)>, Query<(&mut Visibility,), With<Text>>)>,
) {
    if tile_label_query.iter(world).next().is_none() {
        world.run_system_once(spawn_tile_labels).unwrap();
    }

    {
        let (tile_label_query, mut text_query) = system_state.get_mut(world);

        for (tile_label,) in tile_label_query.iter() {
            if let Ok((mut visibility,)) = text_query.get_mut(tile_label.0) {
                *visibility = Visibility::Visible;
            }
        }
    }
}

fn hide_tile_labels(
    tile_label_query: Query<(&TileLabel,)>,
    mut text_query: Query<(&mut Visibility,), With<Text>>,
) {
    for (tile_label,) in tile_label_query.iter() {
        if let Ok((mut visibility,)) = text_query.get_mut(tile_label.0) {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Highlights the tile position label under the cursor.
fn highlight_tile_labels(
    mut commands: Commands,
    cursor_tile_pos: Option<Res<CursorTilePos>>,
    base_terrain_tilemap_query: Query<(&TileStorage,), BaseTerrainLayerFilter>,
    highlighted_base_terrain_tile_query: Query<(Entity,), With<HighlightedLabel>>,
    tile_label_query: Query<(&TileLabel,)>,
    mut text_query: Query<(&mut TextColor,), With<Text2d>>,
) {
    // Un-highlight any previously highlighted tile labels.
    for (tile_entity,) in highlighted_base_terrain_tile_query.iter() {
        if let Ok((label,)) = tile_label_query.get(tile_entity) {
            if let Ok((mut text_color,)) = text_query.get_mut(label.0) {
                *text_color = TextColor(Color::BLACK);
                commands.entity(tile_entity).remove::<HighlightedLabel>();
            }
        }
    }

    let (tile_storage,) = base_terrain_tilemap_query.get_single().unwrap();
    if let Some(cursor_tile_pos) = cursor_tile_pos {
        // Highlight the relevant tile's label
        if let Some(tile_entity) = tile_storage.get(&cursor_tile_pos.0) {
            if let Ok((label,)) = tile_label_query.get(tile_entity) {
                if let Ok((mut text_color,)) = text_query.get_mut(label.0) {
                    *text_color = TextColor(palettes::tailwind::RED_600.into());
                    commands.entity(tile_entity).insert(HighlightedLabel);
                }
            }
        }
    }
}
