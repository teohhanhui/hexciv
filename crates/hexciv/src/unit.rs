use std::any::TypeId;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter;
use std::ops::Add;

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use bevy_ecs_tilemap::helpers::hex_grid::cube::CubePos;
use bevy_ecs_tilemap::helpers::hex_grid::neighbors::{HexNeighbors, HEX_DIRECTIONS};
use bevy_ecs_tilemap::prelude::*;
use bitvec::prelude::*;
use derive_more::Display;
use indexmap::IndexSet;
use itertools::Itertools as _;
use leafwing_input_manager::prelude::*;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ordered_float::NotNan;
use pathfinding::directed::astar::astar;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::action::GlobalAction;
use crate::civilization::Civilization;
use crate::game_setup::GameRng;
use crate::input::CursorTilePos;
use crate::layer::{
    BaseTerrainLayer, BaseTerrainLayerFilter, CivilianUnitLayer, CivilianUnitLayerFilter,
    LandMilitaryUnitLayer, LandMilitaryUnitLayerFilter, LayerZIndex as _, RiverLayer,
    RiverLayerFilter, TerrainFeaturesLayer, TerrainFeaturesLayerFilter, UnitLayersFilter,
    UnitSelectionLayer, UnitSelectionLayerFilter, UnitStateLayer, UnitStateLayerFilter,
};
use crate::peer::{HostBroadcast, Request};
use crate::player::{OurPlayer, Player, PlayerIndex};
use crate::state::{MultiplayerState, TurnState};
use crate::terrain::{BaseTerrain, RiverEdges, TerrainFeatures};
use crate::turn::TurnStarted;

/// A map from [`UnitId`] to [`Entity`] ID.
#[derive(Default, Resource)]
pub struct UnitEntityMap(pub HashMap<UnitId, Entity>);

#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Component, Deserialize, Serialize,
)]
pub struct UnitId(pub Uuid);

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display, Component, Deserialize, Serialize)]
pub enum UnitType {
    Civilian(CivilianUnit),
    LandMilitary(LandMilitaryUnit),
}

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Debug,
    Display,
    Component,
    Deserialize,
    IntoPrimitive,
    Serialize,
    TryFromPrimitive,
)]
#[repr(u32)]
pub enum CivilianUnit {
    Settler = 0,
}

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Debug,
    Display,
    Component,
    Deserialize,
    IntoPrimitive,
    Serialize,
    TryFromPrimitive,
)]
#[repr(u32)]
pub enum LandMilitaryUnit {
    Warrior = 0,
}

#[derive(Copy, Clone, Component)]
pub struct MovementPoints(pub NotNan<f64>);

#[derive(Copy, Clone, Component)]
pub struct FullMovementPoints(pub NotNan<f64>);

#[derive(Copy, Clone, Eq, PartialEq, Component, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum UnitState {
    CivilianReady = 0,
    LandMilitaryReady = 1,
    LandMilitaryFortified = 2,
    CivilianReadyOutOfOrders = 3,
    LandMilitaryReadyOutOfOrders = 4,
    LandMilitaryFortifiedOutOfOrders = 5,
    CivilianOutOfMoves = 6,
    LandMilitaryOutOfMoves = 7,
}

#[derive(Copy, Clone, IntoPrimitive)]
#[repr(u32)]
pub enum UnitStateModifier {
    OutOfOrders = 3,
}

#[derive(Copy, Clone, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum UnitSelection {
    Active = 0,
}

#[derive(Copy, Clone, Eq, PartialEq, Component)]
pub struct UnitEntityId(pub Entity);

#[derive(Component)]
pub struct ActionsLegend;

#[derive(Bundle)]
pub struct UnitBundle {
    pub unit_id: UnitId,
    pub position: TilePos,
    pub unit_type: UnitType,
    pub civ: Civilization,
    pub movement_points: MovementPoints,
    pub full_movement_points: FullMovementPoints,
    pub unit_state: UnitState,
}

#[derive(Bundle)]
pub struct CivilianUnitBundle {
    pub unit_bundle: UnitBundle,
}

#[derive(Bundle)]
pub struct LandMilitaryUnitBundle {
    pub unit_bundle: UnitBundle,
}

#[derive(Bundle)]
pub struct UnitSelectionTileBundle {
    pub tile_bundle: TileBundle,
    pub unit_entity_id: UnitEntityId,
    layer: UnitSelectionLayer,
}

#[derive(Bundle)]
pub struct UnitStateTileBundle {
    pub tile_bundle: TileBundle,
    pub unit_entity_id: UnitEntityId,
    layer: UnitStateLayer,
}

#[derive(Bundle)]
pub struct CivilianUnitTileBundle {
    pub tile_bundle: TileBundle,
    pub unit_entity_id: UnitEntityId,
    layer: CivilianUnitLayer,
}

#[derive(Bundle)]
pub struct LandMilitaryUnitTileBundle {
    pub tile_bundle: TileBundle,
    pub unit_entity_id: UnitEntityId,
    layer: LandMilitaryUnitLayer,
}

pub enum UnitTileBundle {
    Civilian(CivilianUnitTileBundle),
    LandMilitary(LandMilitaryUnitTileBundle),
}

#[derive(QueryFilter)]
pub struct UnitFilter(
    With<UnitType>,
    Without<BaseTerrainLayer>,
    Without<RiverLayer>,
    Without<TerrainFeaturesLayer>,
    Without<UnitSelectionLayer>,
    Without<UnitStateLayer>,
    Without<CivilianUnitLayer>,
    Without<LandMilitaryUnitLayer>,
);

#[derive(Copy, Clone, Debug, Deserialize, Event, Serialize)]
pub struct UnitSpawned {
    pub unit_id: UnitId,
    pub position: TilePos,
    pub unit_type: UnitType,
    pub civ: Civilization,
}

#[derive(Copy, Clone, Debug, Event)]
pub struct UnitSelected {
    pub entity: Entity,
    pub position: TilePos,
}

#[derive(Copy, Clone, Debug, Deserialize, Event, Serialize)]
pub struct UnitMoved {
    pub unit_id: UnitId,
    pub from_pos: TilePos,
    pub to_pos: TilePos,
    pub movement_cost: NotNan<f64>,
}

impl From<Uuid> for UnitId {
    fn from(inner: Uuid) -> Self {
        Self(inner)
    }
}

impl From<CivilianUnit> for UnitType {
    fn from(inner: CivilianUnit) -> Self {
        Self::Civilian(inner)
    }
}

impl From<LandMilitaryUnit> for UnitType {
    fn from(inner: LandMilitaryUnit) -> Self {
        Self::LandMilitary(inner)
    }
}

impl CivilianUnit {
    fn movement_points(&self) -> u32 {
        match self {
            Self::Settler => 2,
        }
    }
}

impl LandMilitaryUnit {
    fn movement_points(&self) -> u32 {
        match self {
            Self::Warrior => 2,
        }
    }
}

impl Add<UnitStateModifier> for UnitState {
    type Output = Self;

    fn add(self, rhs: UnitStateModifier) -> Self::Output {
        match self {
            Self::CivilianReady | Self::LandMilitaryReady | Self::LandMilitaryFortified => {
                let state: u32 = self.into();
                let modifier: u32 = rhs.into();
                Self::try_from(state + modifier).unwrap()
            },
            Self::CivilianReadyOutOfOrders
            | Self::LandMilitaryReadyOutOfOrders
            | Self::LandMilitaryFortifiedOutOfOrders => {
                unimplemented!("unit state modifiers are not stackable");
            },
            Self::CivilianOutOfMoves | Self::LandMilitaryOutOfMoves => {
                unimplemented!("out-of-moves unit states do not have modifiers");
            },
        }
    }
}

impl UnitBundle {
    fn new(
        unit_id: UnitId,
        position: TilePos,
        unit_type: UnitType,
        civ: Civilization,
        movement_points: MovementPoints,
        full_movement_points: FullMovementPoints,
        unit_state: UnitState,
    ) -> Self {
        Self {
            unit_id,
            position,
            unit_type,
            civ,
            movement_points,
            full_movement_points,
            unit_state,
        }
    }
}

impl CivilianUnitBundle {
    pub fn new(
        unit_id: UnitId,
        position: TilePos,
        civ: Civilization,
        civilian_unit: CivilianUnit,
    ) -> Self {
        let (movement_points, full_movement_points) = {
            let movement_points = NotNan::from(civilian_unit.movement_points());
            (
                MovementPoints(movement_points),
                FullMovementPoints(movement_points),
            )
        };
        let unit_state = UnitState::CivilianReady;

        Self {
            unit_bundle: UnitBundle::new(
                unit_id,
                position,
                UnitType::Civilian(civilian_unit),
                civ,
                movement_points,
                full_movement_points,
                unit_state,
            ),
        }
    }
}

impl LandMilitaryUnitBundle {
    pub fn new(
        unit_id: UnitId,
        position: TilePos,
        civ: Civilization,
        land_military_unit: LandMilitaryUnit,
    ) -> Self {
        let (movement_points, full_movement_points) = {
            let movement_points = NotNan::from(land_military_unit.movement_points());
            (
                MovementPoints(movement_points),
                FullMovementPoints(movement_points),
            )
        };
        let unit_state = UnitState::LandMilitaryReady;

        Self {
            unit_bundle: UnitBundle::new(
                unit_id,
                position,
                UnitType::LandMilitary(land_military_unit),
                civ,
                movement_points,
                full_movement_points,
                unit_state,
            ),
        }
    }
}

impl UnitSelectionTileBundle {
    pub fn new(tile_pos: TilePos, tilemap_id: TilemapId, unit_entity_id: UnitEntityId) -> Self {
        Self {
            tile_bundle: TileBundle {
                position: tile_pos,
                tilemap_id,
                texture_index: TileTextureIndex(UnitSelection::Active.into()),
                ..Default::default()
            },
            unit_entity_id,
            layer: UnitSelectionLayer,
        }
    }
}

impl UnitStateTileBundle {
    pub fn new(
        tile_pos: TilePos,
        unit_type: UnitType,
        civ: Civilization,
        tilemap_id: TilemapId,
        unit_entity_id: UnitEntityId,
    ) -> Self {
        let unit_state = match unit_type {
            UnitType::Civilian(_civilian_unit) => UnitState::CivilianReady,
            UnitType::LandMilitary(_land_military_unit) => UnitState::LandMilitaryReady,
        };

        Self {
            tile_bundle: TileBundle {
                position: tile_pos,
                tilemap_id,
                texture_index: TileTextureIndex(unit_state.into()),
                color: TileColor(civ.colors()[0].into()),
                ..Default::default()
            },
            unit_entity_id,
            layer: UnitStateLayer,
        }
    }
}

impl CivilianUnitTileBundle {
    pub fn new(
        tile_pos: TilePos,
        civilian_unit: CivilianUnit,
        civ: Civilization,
        tilemap_id: TilemapId,
        unit_entity_id: UnitEntityId,
    ) -> Self {
        Self {
            tile_bundle: TileBundle {
                position: tile_pos,
                tilemap_id,
                texture_index: TileTextureIndex(civilian_unit.into()),
                color: TileColor(civ.colors()[1].into()),
                ..Default::default()
            },
            unit_entity_id,
            layer: CivilianUnitLayer,
        }
    }
}

impl LandMilitaryUnitTileBundle {
    pub fn new(
        tile_pos: TilePos,
        land_military_unit: LandMilitaryUnit,
        civ: Civilization,
        tilemap_id: TilemapId,
        unit_entity_id: UnitEntityId,
    ) -> Self {
        Self {
            tile_bundle: TileBundle {
                position: tile_pos,
                tilemap_id,
                texture_index: TileTextureIndex(land_military_unit.into()),
                color: TileColor(civ.colors()[1].into()),
                ..Default::default()
            },
            unit_entity_id,
            layer: LandMilitaryUnitLayer,
        }
    }
}

pub fn spawn_starting_units(
    mut game_rng: ResMut<GameRng>,
    player_query: Query<(&PlayerIndex, &Civilization), With<Player>>,
    base_terrain_tilemap_query: Query<(&TilemapSize, &TileStorage), BaseTerrainLayerFilter>,
    base_terrain_tile_query: Query<(&TileTextureIndex,), BaseTerrainLayerFilter>,
    mut turn_started_events: EventWriter<TurnStarted>,
    mut unit_spawned_events: EventWriter<UnitSpawned>,
) {
    let rng = &mut game_rng.0;

    let (map_size, base_terrain_tile_storage) = base_terrain_tilemap_query.get_single().unwrap();

    let mut allowable_starting_positions = HashSet::new();
    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let base_terrain_tile_entity = base_terrain_tile_storage.get(&tile_pos).unwrap();
            let (base_terrain_tile_texture,) = base_terrain_tile_query
                .get(base_terrain_tile_entity)
                .unwrap();

            if [BaseTerrain::Ocean, BaseTerrain::Coast]
                .into_iter()
                .chain(BaseTerrain::MOUNTAINS)
                .map(u32::from)
                .contains(&base_terrain_tile_texture.0)
            {
                continue;
            }

            allowable_starting_positions.insert(tile_pos);
        }
    }

    for (_player_index, &civ) in player_query.iter().sort::<&PlayerIndex>() {
        // TODO: Space out the starting positions for different civs.
        let (settler_tile_pos, warrior_tile_pos) = {
            let rng = RefCell::new(&mut *rng);
            iter::from_fn({
                let rng = &rng;
                let mut allowable_starting_positions = allowable_starting_positions.clone();
                move || {
                    let settler_tile_pos =
                        *rng.borrow_mut().choice(&allowable_starting_positions)?;
                    allowable_starting_positions.remove(&settler_tile_pos);
                    Some(settler_tile_pos)
                }
            })
            .find_map(|settler_tile_pos| {
                let neighbor_positions: HashSet<_> =
                    HexNeighbors::get_neighboring_positions_row_odd(&settler_tile_pos, map_size)
                        .iter()
                        .copied()
                        .collect();
                let allowable_neighbor_positions: HashSet<_> = neighbor_positions
                    .intersection(&allowable_starting_positions)
                    .copied()
                    .collect();
                rng.borrow_mut()
                    .choice(allowable_neighbor_positions)
                    .map(|warrior_tile_pos| (settler_tile_pos, warrior_tile_pos))
            })
            .expect("the map should have enough land tiles to spawn starting units")
        };

        // Spawn settler.
        unit_spawned_events.send(UnitSpawned {
            unit_id: Uuid::new_v4().into(),
            position: settler_tile_pos,
            unit_type: CivilianUnit::Settler.into(),
            civ,
        });

        // Spawn warrior.
        unit_spawned_events.send(UnitSpawned {
            unit_id: Uuid::new_v4().into(),
            position: warrior_tile_pos,
            unit_type: LandMilitaryUnit::Warrior.into(),
            civ,
        });
    }

    turn_started_events.send(TurnStarted { turn_num: 1 });
}

/// Resets movement points for all units.
pub fn reset_movement_points(
    mut unit_query: Query<(&mut MovementPoints, &FullMovementPoints), UnitFilter>,
) {
    for (mut movement_points, full_movement_points) in unit_query.iter_mut() {
        movement_points.0 = full_movement_points.0;
    }
}

/// Checks if there are ready units controlled by the current player.
pub fn has_ready_units(
    our_player: Res<OurPlayer>,
    player_query: Query<(&Civilization,), With<Player>>,
    unit_query: Query<(Entity, &TilePos, &Civilization, &UnitState), UnitFilter>,
) -> bool {
    let (current_civ,) = player_query.get(our_player.0).unwrap();

    unit_query
        .iter()
        .any(|(_unit_entity, _tile_pos, civ, unit_state)| {
            civ == current_civ
                && matches!(
                    unit_state,
                    UnitState::CivilianReady | UnitState::LandMilitaryReady
                )
        })
}

/// Cycles to the previous / next ready unit controlled by the current player.
pub fn cycle_ready_unit(
    global_action_state: Res<ActionState<GlobalAction>>,
    our_player: Res<OurPlayer>,
    player_query: Query<(&Civilization,), With<Player>>,
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex, &UnitEntityId),
        UnitSelectionLayerFilter,
    >,
    unit_query: Query<(Entity, &UnitId, &TilePos, &Civilization, &UnitState), UnitFilter>,
    mut unit_selected_events: EventWriter<UnitSelected>,
) {
    let (current_civ,) = player_query.get(our_player.0).unwrap();

    let ready_units: IndexSet<_> = unit_query
        .iter()
        .sort::<&UnitId>()
        .filter_map(|(unit_entity, _unit_id, tile_pos, civ, unit_state)| {
            if civ == current_civ
                && matches!(
                    unit_state,
                    UnitState::CivilianReady | UnitState::LandMilitaryReady
                )
            {
                Some((unit_entity, *tile_pos))
            } else {
                None
            }
        })
        .collect();
    if ready_units.is_empty() {
        // There are no ready units to cycle to.
        return;
    }
    let active_unit_selection =
        unit_selection_tile_query
            .iter()
            .find(|(_tile_pos, &tile_texture, _unit_entity_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });

    if let Some((_active_unit_tile_pos, _tile_texture, UnitEntityId(active_unit_entity))) =
        active_unit_selection
    {
        // Select the previous / next ready unit.

        let units: Vec<_> = unit_query
            .iter()
            .sort::<&UnitId>()
            .filter_map(|(unit_entity, _unit_id, &tile_pos, civ, _unit_state)| {
                if civ == current_civ {
                    Some((unit_entity, tile_pos))
                } else {
                    None
                }
            })
            .collect();

        if global_action_state.just_pressed(&GlobalAction::PreviousReadyUnit) {
            let previous_units: IndexSet<_> = units
                .into_iter()
                .rev()
                .skip_while(|(unit_entity, _tile_pos)| unit_entity != active_unit_entity)
                .skip(1)
                .collect();
            if let Some((unit_entity, tile_pos)) = previous_units.intersection(&ready_units).next()
            {
                unit_selected_events.send(UnitSelected {
                    entity: *unit_entity,
                    position: *tile_pos,
                });
            }
        } else if global_action_state.just_pressed(&GlobalAction::NextReadyUnit) {
            let next_units: IndexSet<_> = units
                .into_iter()
                .skip_while(|(unit_entity, _tile_pos)| unit_entity != active_unit_entity)
                .skip(1)
                .collect();
            if let Some((unit_entity, tile_pos)) = next_units.intersection(&ready_units).next() {
                unit_selected_events.send(UnitSelected {
                    entity: *unit_entity,
                    position: *tile_pos,
                });
            }
        } else {
            // Not cycling units.
            return;
        }
    } else {
        // Select the first ready unit, since there was no currently active unit.

        let (unit_entity, tile_pos) = ready_units[0];
        unit_selected_events.send(UnitSelected {
            entity: unit_entity,
            position: tile_pos,
        });
    }
}

pub fn focus_camera_on_active_unit(
    mut camera_query: Query<(&mut Transform,), (With<Camera2d>, Without<UnitSelectionLayer>)>,
    unit_selection_tilemap_query: Query<
        (&Transform, &TilemapType, &TilemapGridSize),
        (UnitSelectionLayerFilter, Without<Camera2d>),
    >,
    unit_selection_tile_query: Query<(&TilePos, &TileTextureIndex), UnitSelectionLayerFilter>,
) {
    let (mut camera_transform,) = camera_query.get_single_mut().unwrap();
    let (map_transform, map_type, grid_size) = unit_selection_tilemap_query.get_single().unwrap();

    let active_unit_selection =
        unit_selection_tile_query
            .iter()
            .find(|(_tile_pos, &tile_texture)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });
    if let Some((tile_pos, _tile_texture)) = active_unit_selection {
        let tile_center = tile_pos
            .center_in_world(grid_size, map_type)
            .extend(UnitSelectionLayer::Z_INDEX);
        let tile_translation = map_transform.translation + tile_center;
        camera_transform.translation = tile_translation.with_z(camera_transform.translation.z);
    }
}

pub fn mark_active_unit_out_of_orders(
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex, &UnitEntityId),
        UnitSelectionLayerFilter,
    >,
    mut unit_state_tile_query: Query<(&mut TileTextureIndex,), UnitStateLayerFilter>,
    mut unit_query: Query<(&mut UnitState,), UnitFilter>,
) {
    let (unit_state_tile_storage,) = unit_state_tilemap_query.get_single().unwrap();

    let active_unit_selection =
        unit_selection_tile_query
            .iter()
            .find(|(_tile_pos, &tile_texture, _unit_entity_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });
    let Some((active_unit_tile_pos, _tile_texture, &UnitEntityId(active_unit_entity))) =
        active_unit_selection
    else {
        // No active unit selection.
        return;
    };

    let (mut unit_state,) = unit_query.get_mut(active_unit_entity).unwrap();
    let next_unit_state = match *unit_state {
        UnitState::CivilianReady => UnitState::CivilianReady + UnitStateModifier::OutOfOrders,
        UnitState::LandMilitaryReady => {
            UnitState::LandMilitaryReady + UnitStateModifier::OutOfOrders
        },
        UnitState::LandMilitaryFortified => {
            UnitState::LandMilitaryReady + UnitStateModifier::OutOfOrders
        },
        s if s == UnitState::LandMilitaryFortified + UnitStateModifier::OutOfOrders => {
            UnitState::LandMilitaryReady + UnitStateModifier::OutOfOrders
        },
        _ => {
            // Unit state is unchanged.
            return;
        },
    };
    *unit_state = next_unit_state;

    let (mut tile_texture,) = unit_state_tile_storage
        .get(active_unit_tile_pos)
        .map(|tile_entity| unit_state_tile_query.get_mut(tile_entity).unwrap())
        .expect("active unit tile position should have unit state tile");
    *tile_texture = TileTextureIndex(next_unit_state.into());
}

pub fn mark_active_unit_fortified(
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    land_military_unit_tilemap_query: Query<(&TileStorage,), LandMilitaryUnitLayerFilter>,
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex, &UnitEntityId),
        UnitSelectionLayerFilter,
    >,
    mut unit_state_tile_query: Query<(&mut TileTextureIndex,), UnitStateLayerFilter>,
    mut unit_query: Query<(&mut UnitState,), UnitFilter>,
) {
    let (unit_state_tile_storage,) = unit_state_tilemap_query.get_single().unwrap();
    let (land_military_unit_tile_storage,) = land_military_unit_tilemap_query.get_single().unwrap();

    let active_unit_selection =
        unit_selection_tile_query
            .iter()
            .find(|(_tile_pos, &tile_texture, _unit_entity_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });
    let Some((active_unit_tile_pos, _tile_texture, &UnitEntityId(active_unit_entity))) =
        active_unit_selection
    else {
        // No active unit selection.
        return;
    };

    if land_military_unit_tile_storage
        .get(active_unit_tile_pos)
        .is_none()
    {
        // Active unit is not a land military unit.
        return;
    }

    let (mut unit_state,) = unit_query.get_mut(active_unit_entity).unwrap();
    let next_unit_state = UnitState::LandMilitaryFortified + UnitStateModifier::OutOfOrders;
    unit_state.set_if_neq(next_unit_state);

    let (mut tile_texture,) = unit_state_tile_storage
        .get(active_unit_tile_pos)
        .map(|tile_entity| unit_state_tile_query.get_mut(tile_entity).unwrap())
        .expect("active unit tile position should have unit state tile");
    tile_texture.set_if_neq(TileTextureIndex(next_unit_state.into()));
}

/// Selects the unit at the cursor's tile position controlled by the current
/// player.
#[allow(clippy::too_many_arguments)]
pub fn select_unit(
    our_player: Res<OurPlayer>,
    cursor_tile_pos: Res<CursorTilePos>,
    player_query: Query<(&Civilization,), With<Player>>,
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex, &UnitEntityId),
        UnitSelectionLayerFilter,
    >,
    unit_state_tile_query: Query<(&UnitEntityId,), UnitStateLayerFilter>,
    unit_query: Query<(Entity, &UnitId, &TilePos, &Civilization), UnitFilter>,
    mut unit_selected_events: EventWriter<UnitSelected>,
) {
    let (unit_state_tile_storage,) = unit_state_tilemap_query.get_single().unwrap();

    let (&current_civ,) = player_query.get(our_player.0).unwrap();

    let units: Vec<_> = unit_query
        .iter()
        .sort::<&UnitId>()
        .filter_map(|(unit_entity, _unit_id, &tile_pos, &civ)| {
            if civ == current_civ && tile_pos == cursor_tile_pos.0 {
                Some((unit_entity, tile_pos))
            } else {
                None
            }
        })
        .collect();
    if units.is_empty() {
        // No selectable unit present at this tile position.
        return;
    }

    let active_unit_selection =
        unit_selection_tile_query
            .iter()
            .find(|(_tile_pos, &tile_texture, _unit_entity_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });

    if let Some((active_unit_tile_pos, _tile_texture, UnitEntityId(active_unit_entity))) =
        active_unit_selection
            .filter(|(&tile_pos, _tile_texture, _unit_entity_id)| tile_pos == cursor_tile_pos.0)
    {
        // Select the next unit at the same tile position as the active unit.
        // This allows cycling through stacked units.

        let next_units = units
            .iter()
            .skip_while(|(unit_entity, _tile_pos)| unit_entity != active_unit_entity)
            .skip(1);
        let previous_units = units
            .iter()
            .take_while(|(unit_entity, _tile_pos)| unit_entity != active_unit_entity);

        if let Some(&(unit_entity, tile_pos)) = next_units.chain(previous_units).next() {
            unit_selected_events.send(UnitSelected {
                entity: unit_entity,
                position: tile_pos,
            });
        } else {
            // Re-select the active unit.
            unit_selected_events.send(UnitSelected {
                entity: *active_unit_entity,
                position: *active_unit_tile_pos,
            });
        }
    } else {
        // Select the unit whose unit state is shown at this tile position.

        let tile_entity = unit_state_tile_storage
            .get(&cursor_tile_pos.0)
            .expect("tile position with units should have unit state tile");
        let (&UnitEntityId(unit_entity),) = unit_state_tile_query.get(tile_entity).unwrap();

        unit_selected_events.send(UnitSelected {
            entity: unit_entity,
            position: cursor_tile_pos.0,
        });
    }
}

pub fn should_move_active_unit_to(
    cursor_tile_pos: Res<CursorTilePos>,
    unit_selection_tile_query: Query<(&TilePos, &TileTextureIndex), UnitSelectionLayerFilter>,
) -> bool {
    let active_unit_selection =
        unit_selection_tile_query
            .iter()
            .find(|(_tile_pos, &tile_texture)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });
    let Some((&active_unit_tile_pos, _tile_texture)) = active_unit_selection else {
        // Nothing to move as there is no active unit selection.
        return false;
    };

    if cursor_tile_pos.0 == active_unit_tile_pos {
        // Active unit is already in the selected tile.
        return false;
    }

    true
}

#[allow(clippy::too_many_arguments)]
pub fn move_active_unit_to(
    cursor_tile_pos: Res<CursorTilePos>,
    multiplayer_state: Res<State<MultiplayerState>>,
    base_terrain_tilemap_query: Query<(&TilemapSize, &TileStorage), BaseTerrainLayerFilter>,
    river_tilemap_query: Query<(&TileStorage,), RiverLayerFilter>,
    terrain_features_tilemap_query: Query<(&TileStorage,), TerrainFeaturesLayerFilter>,
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    base_terrain_tile_query: Query<(&TileTextureIndex,), BaseTerrainLayerFilter>,
    river_tile_query: Query<(&TileTextureIndex,), RiverLayerFilter>,
    terrain_features_tile_query: Query<(&TileTextureIndex,), TerrainFeaturesLayerFilter>,
    unit_selection_tile_query: Query<(&TilePos, &TileTextureIndex), UnitSelectionLayerFilter>,
    unit_state_tile_query: Query<(&UnitEntityId,), UnitStateLayerFilter>,
    unit_query: Query<(&UnitId, &MovementPoints, &FullMovementPoints), UnitFilter>,
    mut request_events: EventWriter<Request>,
    mut unit_moved_events: EventWriter<UnitMoved>,
) {
    let (map_size, base_terrain_tile_storage) = base_terrain_tilemap_query.get_single().unwrap();
    let (river_tile_storage,) = river_tilemap_query.get_single().unwrap();
    let (terrain_features_tile_storage,) = terrain_features_tilemap_query.get_single().unwrap();
    let (unit_state_tile_storage,) = unit_state_tilemap_query.get_single().unwrap();

    let active_unit_selection_pos = unit_selection_tile_query
        .iter()
        .find_map(|(tile_pos, &tile_texture)| {
            if matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            {
                Some(*tile_pos)
            } else {
                None
            }
        })
        .expect("there should be an active unit selection");
    let start = active_unit_selection_pos;
    let goal = cursor_tile_pos.0;
    let (&UnitEntityId(unit_entity),) = unit_state_tile_storage
        .get(&start)
        .map(|tile_entity| unit_state_tile_query.get(tile_entity).unwrap())
        .expect("active unit tile position should have unit state tile");
    let (&unit_id, movement_points, full_movement_points) = unit_query.get(unit_entity).unwrap();

    let successors = |(x, y)| {
        let tile_pos = TilePos { x, y };
        let neighbor_positions =
            HexNeighbors::get_neighboring_positions_row_odd(&tile_pos, map_size);
        let neighbor_positions_map: BTreeMap<_, _> = HEX_DIRECTIONS
            .into_iter()
            .filter_map(move |direction| {
                neighbor_positions
                    .get(direction)
                    .map(|tile_pos| (direction, *tile_pos))
            })
            .collect();
        let river_edges: RiverEdges = river_tile_storage
            .get(&tile_pos)
            .map(|tile_entity| river_tile_query.get(tile_entity).unwrap())
            .map_or(BitArray::<_>::ZERO, |(tile_texture,)| {
                let mut river_edges: RiverEdges = BitArray::<_>::ZERO;
                river_edges.store(tile_texture.0);
                river_edges
            });

        neighbor_positions_map.into_iter().filter_map({
            #[allow(clippy::borrow_deref_ref)]
            let base_terrain_tile_storage = &*base_terrain_tile_storage;
            #[allow(clippy::borrow_deref_ref)]
            let terrain_features_tile_storage = &*terrain_features_tile_storage;
            let base_terrain_tile_query = base_terrain_tile_query.to_readonly();
            let terrain_features_tile_query = terrain_features_tile_query.to_readonly();
            move |(direction, tile_pos)| {
                let (base_terrain_tile_texture,) = {
                    let tile_entity = base_terrain_tile_storage.get(&tile_pos).unwrap();
                    base_terrain_tile_query.get(tile_entity).unwrap()
                };
                let base_terrain = BaseTerrain::try_from(base_terrain_tile_texture.0).unwrap();
                if base_terrain.is_mountains() {
                    return None;
                }
                // TODO: Conditionally allow units to embark.
                if [BaseTerrain::Ocean, BaseTerrain::Coast].contains(&base_terrain) {
                    return None;
                }
                let terrain_features_tile_texture = terrain_features_tile_storage
                    .get(&tile_pos)
                    .map(|tile_entity| terrain_features_tile_query.get(tile_entity).unwrap())
                    .map(|(tile_texture,)| *tile_texture);
                let movement_cost = if base_terrain.is_hills() {
                    match terrain_features_tile_texture {
                        Some(TileTextureIndex(t)) if t == u32::from(TerrainFeatures::Woods) => {
                            NotNan::from(3)
                        },
                        Some(TileTextureIndex(t))
                            if t == u32::from(TerrainFeatures::Rainforest) =>
                        {
                            NotNan::from(3)
                        },
                        _ => NotNan::from(2),
                    }
                } else {
                    match terrain_features_tile_texture {
                        Some(TileTextureIndex(t)) if t == u32::from(TerrainFeatures::Woods) => {
                            NotNan::from(2)
                        },
                        Some(TileTextureIndex(t))
                            if t == u32::from(TerrainFeatures::Rainforest) =>
                        {
                            NotNan::from(2)
                        },
                        Some(TileTextureIndex(t)) if t == u32::from(TerrainFeatures::Marsh) => {
                            NotNan::from(2)
                        },
                        _ => NotNan::from(1),
                    }
                };
                let movement_cost = if river_edges[direction as usize] {
                    movement_cost + NotNan::from(3)
                } else {
                    movement_cost
                };

                let TilePos { x, y } = tile_pos;
                Some(((x, y), movement_cost))
            }
        })
    };

    let mut current = start;
    let mut movement_points = *movement_points;
    while current != goal {
        // TODO: Limit pathfinding to partial knowledge:
        // 1. Only tiles already explored by the current player would have a known
        //    movement cost.
        // 2. Only tiles already explored by the current player would have known
        //    presence / absence of neighboring tiles. If the neighboring tile positions
        //    have never been inside any unit's sight range, they must be assumed to
        //    exist.
        // 3. If there are any changes allowing / denying movement since the last seen
        //    time, the changes must NOT be taken into consideration. Pathfinding must
        //    be based on the last known map by the current player.

        let shortest_path = astar(
            &(current.x, current.y),
            |&p| successors(p),
            |&(x, y)| NotNan::from(CubePos::from(TilePos { x, y }).distance_from(&goal.into())),
            |&(x, y)| TilePos { x, y } == goal,
        );

        if let Some((path, _total_movement_cost)) = shortest_path {
            let next = path[1];
            let movement_cost = successors(path[0])
                .find_map(|(p, c)| if p == next { Some(c) } else { None })
                .unwrap();
            if movement_cost <= movement_points.0 {
                movement_points.0 -= movement_cost;
            } else if movement_points.0 == full_movement_points.0 {
                movement_points.0 = NotNan::from(0);
            } else {
                // Not enough movement points.
                // TODO: Queue movement for next turns.
                break;
            }
            let next = {
                let (x, y) = next;
                TilePos { x, y }
            };
            let unit_moved = UnitMoved {
                unit_id,
                from_pos: current,
                to_pos: next,
                movement_cost,
            };
            match multiplayer_state.get() {
                MultiplayerState::Hosting => {
                    unit_moved_events.send(unit_moved);
                },
                MultiplayerState::Joining => {
                    request_events.send(unit_moved.into());
                },
                _ => {
                    unreachable!("multiplayer state should not be inactive");
                },
            }
            current = next;
        } else {
            info!(?current, ?start, ?goal, "could not find path");
            // TODO: Show indication that there is no path for this move.
            break;
        }
    }
}

/// Handles [`UnitSpawned`] events.
#[allow(clippy::too_many_arguments)]
pub fn handle_unit_spawned(
    mut commands: Commands,
    mut unit_entity_map: ResMut<UnitEntityMap>,
    multiplayer_state: Res<State<MultiplayerState>>,
    mut unit_state_tilemap_query: Query<(Entity, &mut TileStorage), UnitStateLayerFilter>,
    mut civilian_unit_tilemap_query: Query<(Entity, &mut TileStorage), CivilianUnitLayerFilter>,
    mut land_military_unit_tilemap_query: Query<
        (Entity, &mut TileStorage),
        LandMilitaryUnitLayerFilter,
    >,
    mut host_broadcast_events: EventWriter<HostBroadcast>,
    mut unit_spawned_events: EventReader<UnitSpawned>,
) {
    let (unit_state_tilemap_entity, mut unit_state_tile_storage) =
        unit_state_tilemap_query.get_single_mut().unwrap();
    let (civilian_unit_tilemap_entity, mut civilian_unit_tile_storage) =
        civilian_unit_tilemap_query.get_single_mut().unwrap();
    let (land_military_unit_tilemap_entity, mut land_military_unit_tile_storage) =
        land_military_unit_tilemap_query.get_single_mut().unwrap();

    for &unit_spawned in unit_spawned_events.read() {
        debug!(?unit_spawned, "handling unit spawned");
        let UnitSpawned {
            unit_id,
            position,
            unit_type,
            civ,
        } = unit_spawned;

        match unit_type {
            UnitType::Civilian(civilian_unit) => {
                let unit_entity = commands
                    .spawn(CivilianUnitBundle::new(
                        unit_id,
                        position,
                        civ,
                        civilian_unit,
                    ))
                    .id();
                unit_entity_map.0.insert(unit_id, unit_entity);
                let tile_entity = commands
                    .spawn(CivilianUnitTileBundle::new(
                        position,
                        civilian_unit,
                        civ,
                        TilemapId(civilian_unit_tilemap_entity),
                        UnitEntityId(unit_entity),
                    ))
                    .id();
                civilian_unit_tile_storage.set(&position, tile_entity);
                let tile_entity = commands
                    .spawn(UnitStateTileBundle::new(
                        position,
                        civilian_unit.into(),
                        civ,
                        TilemapId(unit_state_tilemap_entity),
                        UnitEntityId(unit_entity),
                    ))
                    .id();
                unit_state_tile_storage.set(&position, tile_entity);
            },
            UnitType::LandMilitary(land_military_unit) => {
                let unit_entity = commands
                    .spawn(LandMilitaryUnitBundle::new(
                        unit_id,
                        position,
                        civ,
                        land_military_unit,
                    ))
                    .id();
                unit_entity_map.0.insert(unit_id, unit_entity);
                let tile_entity = commands
                    .spawn(LandMilitaryUnitTileBundle::new(
                        position,
                        land_military_unit,
                        civ,
                        TilemapId(land_military_unit_tilemap_entity),
                        UnitEntityId(unit_entity),
                    ))
                    .id();
                land_military_unit_tile_storage.set(&position, tile_entity);
                let tile_entity = commands
                    .spawn(UnitStateTileBundle::new(
                        position,
                        land_military_unit.into(),
                        civ,
                        TilemapId(unit_state_tilemap_entity),
                        UnitEntityId(unit_entity),
                    ))
                    .id();
                unit_state_tile_storage.set(&position, tile_entity);
            },
        }

        if matches!(multiplayer_state.get(), MultiplayerState::Hosting) {
            host_broadcast_events.send(unit_spawned.into());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_unit_selected(
    mut commands: Commands,
    turn_state: Res<State<TurnState>>,
    mut unit_selection_tilemap_query: Query<(Entity, &mut TileStorage), UnitSelectionLayerFilter>,
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    mut civilian_unit_tilemap_query: Query<(Entity, &mut TileStorage), CivilianUnitLayerFilter>,
    mut land_military_unit_tilemap_query: Query<
        (Entity, &mut TileStorage),
        LandMilitaryUnitLayerFilter,
    >,
    mut unit_selection_tile_query: Query<
        (Entity, &mut TilePos, &TileTextureIndex, &mut UnitEntityId),
        UnitSelectionLayerFilter,
    >,
    mut unit_state_tile_query: Query<
        (&mut TileTextureIndex, &mut UnitEntityId),
        UnitStateLayerFilter,
    >,
    mut unit_tile_query: Query<(&mut TileTextureIndex, &mut UnitEntityId), UnitLayersFilter>,
    unit_query: Query<(&UnitType, &Civilization, &UnitState), UnitFilter>,
    mut actions_legend_text_query: Query<(&mut Text,), With<ActionsLegend>>,
    mut unit_selected_events: EventReader<UnitSelected>,
) {
    let (unit_selection_tilemap_entity, mut unit_selection_tile_storage) =
        unit_selection_tilemap_query.get_single_mut().unwrap();
    let (unit_state_tile_storage,) = unit_state_tilemap_query.get_single().unwrap();
    let (civilian_unit_tilemap_entity, mut civilian_unit_tile_storage) =
        civilian_unit_tilemap_query.get_single_mut().unwrap();
    let (land_military_unit_tilemap_entity, mut land_military_unit_tile_storage) =
        land_military_unit_tilemap_query.get_single_mut().unwrap();
    let (mut actions_legend_text,) = actions_legend_text_query.get_single_mut().unwrap();

    let mut new_unit_selection_tile_bundle = None;
    let mut new_unit_tile_bundles = HashMap::new();
    for &unit_selected in unit_selected_events.read() {
        debug!(?unit_selected, "handling unit selected");
        let UnitSelected {
            entity: selected_unit_entity,
            position: selected_unit_tile_pos,
        } = unit_selected;

        let (&unit_type, &civ, &unit_state) = unit_query.get(selected_unit_entity).unwrap();
        let mut unit_actions_msg = "".to_owned();

        // Update unit selection tile.
        let active_unit_selection = unit_selection_tile_query.iter_mut().find(
            |(_tile_entity, _tile_pos, &tile_texture, _unit_entity_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            },
        );
        new_unit_selection_tile_bundle = None;
        if let Some((tile_entity, mut tile_pos, _tile_texture, mut unit_entity_id)) =
            active_unit_selection
        {
            // Update unit selection tile.

            unit_selection_tile_storage.remove(&tile_pos);
            tile_pos.set_if_neq(selected_unit_tile_pos);
            unit_selection_tile_storage.set(&selected_unit_tile_pos, tile_entity);
            unit_entity_id.set_if_neq(UnitEntityId(selected_unit_entity));
        } else {
            // We need to spawn a new unit selection tile, since there was no currently
            // active unit.

            new_unit_selection_tile_bundle = Some(UnitSelectionTileBundle::new(
                selected_unit_tile_pos,
                TilemapId(unit_selection_tilemap_entity),
                UnitEntityId(selected_unit_entity),
            ));
        }

        // Update unit state tile.
        // This should always exist so long as there is a unit at this tile position,
        // even in cases of unit stacking.
        {
            let (mut tile_texture, mut unit_entity_id) = unit_state_tile_storage
                .get(&selected_unit_tile_pos)
                .map(|tile_entity| unit_state_tile_query.get_mut(tile_entity).unwrap())
                .expect("selected unit tile position should have unit state tile");
            tile_texture.set_if_neq(TileTextureIndex(unit_state.into()));
            unit_entity_id.set_if_neq(UnitEntityId(selected_unit_entity));
        }

        // Update unit tile.
        let mut unit_tile_storages = HashMap::from([
            (
                TypeId::of::<CivilianUnit>(),
                civilian_unit_tile_storage.reborrow(),
            ),
            (
                TypeId::of::<LandMilitaryUnit>(),
                land_military_unit_tile_storage.reborrow(),
            ),
        ]);
        new_unit_tile_bundles.remove(&selected_unit_tile_pos);
        match unit_type {
            UnitType::Civilian(civilian_unit) => {
                let tile_storage = unit_tile_storages
                    .remove(&TypeId::of::<CivilianUnit>())
                    .unwrap();
                update_civilian_unit_tile(
                    &selected_unit_tile_pos,
                    civilian_unit,
                    civ,
                    TilemapId(civilian_unit_tilemap_entity),
                    UnitEntityId(selected_unit_entity),
                    &tile_storage,
                    &mut unit_tile_query,
                    &mut new_unit_tile_bundles,
                );
            },
            UnitType::LandMilitary(land_military_unit) => {
                let tile_storage = unit_tile_storages
                    .remove(&TypeId::of::<LandMilitaryUnit>())
                    .unwrap();
                update_land_military_unit_tile(
                    &selected_unit_tile_pos,
                    land_military_unit,
                    civ,
                    TilemapId(land_military_unit_tilemap_entity),
                    UnitEntityId(selected_unit_entity),
                    &tile_storage,
                    &mut unit_tile_query,
                    &mut new_unit_tile_bundles,
                );
                if matches!(turn_state.get(), TurnState::InProgress) {
                    unit_actions_msg += "[F] Fortify\n";
                }
            },
        }
        // Remove other unit tiles at the same tile position.
        for mut tile_storage in unit_tile_storages.into_values() {
            if let Some(tile_entity) = tile_storage.get(&selected_unit_tile_pos) {
                commands.entity(tile_entity).despawn();
                tile_storage.remove(&selected_unit_tile_pos);
            }
        }

        if matches!(turn_state.get(), TurnState::InProgress) {
            unit_actions_msg += "[Space] Skip Turn\n";
        }
        actions_legend_text.sections[0].value = unit_actions_msg;
    }

    // Do the deferred spawning.
    if let Some(unit_selection_tile_bundle) = new_unit_selection_tile_bundle {
        let tile_pos = unit_selection_tile_bundle.tile_bundle.position;
        let tile_entity = commands.spawn(unit_selection_tile_bundle).id();
        unit_selection_tile_storage.set(&tile_pos, tile_entity);
    }
    for (tile_pos, unit_tile_bundle) in new_unit_tile_bundles {
        match unit_tile_bundle {
            UnitTileBundle::Civilian(civilian_unit_tile_bundle) => {
                let tile_entity = commands.spawn(civilian_unit_tile_bundle).id();
                civilian_unit_tile_storage.set(&tile_pos, tile_entity);
            },
            UnitTileBundle::LandMilitary(land_military_unit_tile_bundle) => {
                let tile_entity = commands.spawn(land_military_unit_tile_bundle).id();
                land_military_unit_tile_storage.set(&tile_pos, tile_entity);
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn handle_unit_moved(
    mut commands: Commands,
    unit_entity_map: Res<UnitEntityMap>,
    multiplayer_state: Res<State<MultiplayerState>>,
    mut unit_selection_tilemap_query: Query<(&mut TileStorage,), UnitSelectionLayerFilter>,
    mut unit_state_tilemap_query: Query<(Entity, &mut TileStorage), UnitStateLayerFilter>,
    mut civilian_unit_tilemap_query: Query<(Entity, &mut TileStorage), CivilianUnitLayerFilter>,
    mut land_military_unit_tilemap_query: Query<
        (Entity, &mut TileStorage),
        LandMilitaryUnitLayerFilter,
    >,
    mut unit_selection_tile_query: Query<
        (Entity, &mut TilePos, &TileTextureIndex, &UnitEntityId),
        UnitSelectionLayerFilter,
    >,
    mut unit_state_tile_query: Query<
        (&mut TileTextureIndex, &mut UnitEntityId),
        UnitStateLayerFilter,
    >,
    mut unit_tile_query: Query<(&mut TileTextureIndex, &mut UnitEntityId), UnitLayersFilter>,
    mut unit_query: Query<
        (
            Entity,
            &UnitId,
            &mut TilePos,
            &UnitType,
            &Civilization,
            &mut MovementPoints,
            &mut UnitState,
        ),
        UnitFilter,
    >,
    mut host_broadcast_events: EventWriter<HostBroadcast>,
    mut unit_moved_events: EventReader<UnitMoved>,
) {
    let (mut unit_selection_tile_storage,) = unit_selection_tilemap_query.get_single_mut().unwrap();
    let (unit_state_tilemap_entity, mut unit_state_tile_storage) =
        unit_state_tilemap_query.get_single_mut().unwrap();
    let (civilian_unit_tilemap_entity, mut civilian_unit_tile_storage) =
        civilian_unit_tilemap_query.get_single_mut().unwrap();
    let (land_military_unit_tilemap_entity, mut land_military_unit_tile_storage) =
        land_military_unit_tilemap_query.get_single_mut().unwrap();

    let mut new_unit_tile_bundles = HashMap::new();
    let mut new_unit_state_tile_bundles = HashMap::new();

    for &unit_moved in unit_moved_events.read() {
        debug!(?unit_moved, "handling unit moved");
        let UnitMoved {
            unit_id: moved_unit_id,
            from_pos,
            to_pos,
            movement_cost,
        } = unit_moved;

        let moved_unit_entity = unit_entity_map
            .0
            .get(&moved_unit_id)
            .expect("unit id of the unit being moved should be associated with an existing entity");

        // Update unit.
        {
            let (
                _unit_entity,
                _unit_id,
                mut tile_pos,
                _unit_type,
                _civ,
                mut movement_points,
                mut unit_state,
            ) = unit_query.get_mut(*moved_unit_entity).unwrap();
            assert!(*tile_pos == from_pos);
            *tile_pos = to_pos;
            movement_points.0 -= *movement_cost;
            if movement_points.0 == 0.0 {
                let next_unit_state = match *unit_state {
                    UnitState::CivilianReady | UnitState::CivilianReadyOutOfOrders => {
                        UnitState::CivilianOutOfMoves
                    },
                    UnitState::LandMilitaryReady
                    | UnitState::LandMilitaryReadyOutOfOrders
                    | UnitState::LandMilitaryFortified
                    | UnitState::LandMilitaryFortifiedOutOfOrders => {
                        UnitState::LandMilitaryOutOfMoves
                    },
                    UnitState::CivilianOutOfMoves | UnitState::LandMilitaryOutOfMoves => {
                        unreachable!("the unit being moved should not be out of moves");
                    },
                };
                *unit_state = next_unit_state;
            }
        }

        for (tile_pos, unit_entity) in [(from_pos, None), (to_pos, Some(moved_unit_entity))] {
            new_unit_tile_bundles.remove(&tile_pos);
            new_unit_state_tile_bundles.remove(&tile_pos);

            let mut unit_tile_storages = HashMap::from([
                (
                    TypeId::of::<CivilianUnit>(),
                    civilian_unit_tile_storage.reborrow(),
                ),
                (
                    TypeId::of::<LandMilitaryUnit>(),
                    land_military_unit_tile_storage.reborrow(),
                ),
            ]);
            let Some((
                unit_entity,
                _unit_id,
                _tile_pos,
                &unit_type,
                civ,
                _movement_points,
                unit_state,
            )) = unit_entity
                .map(|&unit_entity| unit_query.get(unit_entity).unwrap())
                .or_else(|| {
                    unit_query.iter().sort::<&UnitId>().find(
                        |(
                            _unit_entity,
                            _unit_id,
                            &unit_tile_pos,
                            _unit_type,
                            _civ,
                            _movement_points,
                            _unit_state,
                        )| { unit_tile_pos == tile_pos },
                    )
                })
            else {
                // Remove unit tiles and unit state tile, as there are no units at this tile
                // position.
                for mut tile_storage in unit_tile_storages.into_values() {
                    if let Some(tile_entity) = tile_storage.get(&tile_pos) {
                        commands.entity(tile_entity).despawn();
                        tile_storage.remove(&tile_pos);
                    }
                }
                if let Some(tile_entity) = unit_state_tile_storage.get(&tile_pos) {
                    commands.entity(tile_entity).despawn();
                    unit_state_tile_storage.remove(&tile_pos);
                }
                continue;
            };

            match unit_type {
                UnitType::Civilian(civilian_unit) => {
                    // Update unit tile.
                    let tile_storage = unit_tile_storages
                        .remove(&TypeId::of::<CivilianUnit>())
                        .unwrap();
                    update_civilian_unit_tile(
                        &tile_pos,
                        civilian_unit,
                        *civ,
                        TilemapId(civilian_unit_tilemap_entity),
                        UnitEntityId(unit_entity),
                        &tile_storage,
                        &mut unit_tile_query,
                        &mut new_unit_tile_bundles,
                    );

                    // Update unit state tile.
                    if let Some(tile_entity) = unit_state_tile_storage.get(&tile_pos) {
                        let (mut tile_texture, mut unit_entity_id) =
                            unit_state_tile_query.get_mut(tile_entity).unwrap();
                        tile_texture.set_if_neq(TileTextureIndex((*unit_state).into()));
                        *unit_entity_id = UnitEntityId(unit_entity);
                    } else {
                        let mut unit_state_tile_bundle = UnitStateTileBundle::new(
                            tile_pos,
                            UnitType::Civilian(civilian_unit),
                            *civ,
                            TilemapId(unit_state_tilemap_entity),
                            UnitEntityId(unit_entity),
                        );
                        unit_state_tile_bundle.tile_bundle.texture_index =
                            TileTextureIndex((*unit_state).into());
                        new_unit_state_tile_bundles.insert(tile_pos, unit_state_tile_bundle);
                    }
                },
                UnitType::LandMilitary(land_military_unit) => {
                    // Update unit tile.
                    let tile_storage = unit_tile_storages
                        .remove(&TypeId::of::<LandMilitaryUnit>())
                        .unwrap();
                    update_land_military_unit_tile(
                        &tile_pos,
                        land_military_unit,
                        *civ,
                        TilemapId(land_military_unit_tilemap_entity),
                        UnitEntityId(unit_entity),
                        &tile_storage,
                        &mut unit_tile_query,
                        &mut new_unit_tile_bundles,
                    );

                    // Update unit state tile.
                    if let Some(tile_entity) = unit_state_tile_storage.get(&tile_pos) {
                        let (mut tile_texture, mut unit_entity_id) =
                            unit_state_tile_query.get_mut(tile_entity).unwrap();
                        tile_texture.set_if_neq(TileTextureIndex((*unit_state).into()));
                        *unit_entity_id = UnitEntityId(unit_entity);
                    } else {
                        let mut unit_state_tile_bundle = UnitStateTileBundle::new(
                            tile_pos,
                            UnitType::LandMilitary(land_military_unit),
                            *civ,
                            TilemapId(unit_state_tilemap_entity),
                            UnitEntityId(unit_entity),
                        );
                        unit_state_tile_bundle.tile_bundle.texture_index =
                            TileTextureIndex((*unit_state).into());
                        new_unit_state_tile_bundles.insert(tile_pos, unit_state_tile_bundle);
                    }
                },
            }
            // Remove other unit tiles at the same tile position.
            for mut tile_storage in unit_tile_storages.into_values() {
                if let Some(tile_entity) = tile_storage.get(&tile_pos) {
                    commands.entity(tile_entity).despawn();
                    tile_storage.remove(&tile_pos);
                }
            }
        }

        // Update unit selection tile.
        let active_unit_selection =
        unit_selection_tile_query
            .iter_mut()
            .find(|(_tile_entity, _tile_pos, &tile_texture, _unit_entity_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });
        if let Some((tile_entity, mut tile_pos, _tile_texture, _unit_entity_id)) =
            active_unit_selection.filter(
                |(_tile_entity, _tile_pos, _tile_texture, UnitEntityId(unit_entity))| {
                    unit_entity == moved_unit_entity
                },
            )
        {
            assert!(*tile_pos == from_pos);
            unit_selection_tile_storage.remove(&from_pos);
            *tile_pos = to_pos;
            unit_selection_tile_storage.set(&to_pos, tile_entity);
        }

        if matches!(multiplayer_state.get(), MultiplayerState::Hosting) {
            host_broadcast_events.send(unit_moved.into());
        }
    }

    // Do the deferred spawning.
    for (tile_pos, unit_tile_bundle) in new_unit_tile_bundles {
        match unit_tile_bundle {
            UnitTileBundle::Civilian(civilian_unit_tile_bundle) => {
                let tile_entity = commands.spawn(civilian_unit_tile_bundle).id();
                civilian_unit_tile_storage.set(&tile_pos, tile_entity);
            },
            UnitTileBundle::LandMilitary(land_military_unit_tile_bundle) => {
                let tile_entity = commands.spawn(land_military_unit_tile_bundle).id();
                land_military_unit_tile_storage.set(&tile_pos, tile_entity);
            },
        }
    }
    for (tile_pos, unit_state_tile_bundle) in new_unit_state_tile_bundles {
        let tile_entity = commands.spawn(unit_state_tile_bundle).id();
        unit_state_tile_storage.set(&tile_pos, tile_entity);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_civilian_unit_tile(
    tile_pos: &TilePos,
    civilian_unit: CivilianUnit,
    civ: Civilization,
    civilian_unit_tilemap_id: TilemapId,
    civilian_unit_entity_id: UnitEntityId,
    civilian_unit_tile_storage: &TileStorage,
    unit_tile_query: &mut Query<(&mut TileTextureIndex, &mut UnitEntityId), UnitLayersFilter>,
    new_unit_tile_bundles: &mut HashMap<TilePos, UnitTileBundle>,
) {
    if let Some(tile_entity) = civilian_unit_tile_storage.get(tile_pos) {
        let (mut tile_texture, mut unit_entity_id) = unit_tile_query.get_mut(tile_entity).unwrap();
        tile_texture.set_if_neq(TileTextureIndex(civilian_unit.into()));
        unit_entity_id.set_if_neq(civilian_unit_entity_id);
    } else {
        new_unit_tile_bundles.insert(
            *tile_pos,
            UnitTileBundle::Civilian(CivilianUnitTileBundle::new(
                *tile_pos,
                civilian_unit,
                civ,
                civilian_unit_tilemap_id,
                civilian_unit_entity_id,
            )),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn update_land_military_unit_tile(
    tile_pos: &TilePos,
    land_military_unit: LandMilitaryUnit,
    civ: Civilization,
    land_military_unit_tilemap_id: TilemapId,
    land_military_unit_entity_id: UnitEntityId,
    land_military_unit_tile_storage: &TileStorage,
    unit_tile_query: &mut Query<(&mut TileTextureIndex, &mut UnitEntityId), UnitLayersFilter>,
    new_unit_tile_bundles: &mut HashMap<TilePos, UnitTileBundle>,
) {
    if let Some(tile_entity) = land_military_unit_tile_storage.get(tile_pos) {
        let (mut tile_texture, mut unit_entity_id) = unit_tile_query.get_mut(tile_entity).unwrap();
        tile_texture.set_if_neq(TileTextureIndex(land_military_unit.into()));
        unit_entity_id.set_if_neq(land_military_unit_entity_id);
    } else {
        new_unit_tile_bundles.insert(
            *tile_pos,
            UnitTileBundle::LandMilitary(LandMilitaryUnitTileBundle::new(
                *tile_pos,
                land_military_unit,
                civ,
                land_military_unit_tilemap_id,
                land_military_unit_entity_id,
            )),
        );
    }
}
