use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::layer::BaseTerrainLayerFilter;

#[derive(Debug, Resource)]
pub struct CursorPos(pub Vec2);

#[derive(Debug, Resource)]
pub struct CursorTilePos(pub TilePos);

impl Default for CursorPos {
    fn default() -> Self {
        // Initialize the cursor pos at some far away place. It will get updated
        // correctly when the cursor moves.
        Self(Vec2::new(-1000.0, -1000.0))
    }
}

/// Keeps the cursor position updated based on any [`CursorMoved`] events.
pub fn update_cursor_pos(
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
pub fn update_cursor_tile_pos(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    tilemap_query: Query<
        (&TilemapSize, &TilemapGridSize, &TilemapType, &Transform),
        BaseTerrainLayerFilter,
    >,
) {
    let (map_size, grid_size, map_type, map_transform) = tilemap_query.get_single().unwrap();
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
        commands.insert_resource(CursorTilePos(tile_pos));
    } else {
        // Cursor is not hovering over any tile.
        commands.remove_resource::<CursorTilePos>();
    }
}
