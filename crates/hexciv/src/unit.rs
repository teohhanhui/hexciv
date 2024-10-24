use std::any::TypeId;
use std::collections::HashMap;
use std::ops::Add;

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use derive_more::Display;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::civilization::Civilization;
use crate::layer::{
    BaseTerrainLayer, CivilianUnitLayer, CivilianUnitLayerFilter, LandMilitaryUnitLayer,
    LandMilitaryUnitLayerFilter, RiverLayer, TerrainFeaturesLayer, UnitLayersFilter,
    UnitSelectionLayer, UnitSelectionLayerFilter, UnitStateLayer, UnitStateLayerFilter,
};
use crate::peer::{NetworkEntityMap, NetworkId};
use crate::state::TurnState;

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
pub struct UnitId(pub Entity);

#[derive(Component)]
pub struct ActionsLegend;

#[derive(Bundle)]
pub struct UnitBundle {
    pub network_id: NetworkId,
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
    pub unit_id: UnitId,
    layer: UnitSelectionLayer,
}

#[derive(Bundle)]
pub struct UnitStateTileBundle {
    pub tile_bundle: TileBundle,
    pub unit_id: UnitId,
    layer: UnitStateLayer,
}

#[derive(Bundle)]
pub struct CivilianUnitTileBundle {
    pub tile_bundle: TileBundle,
    pub unit_id: UnitId,
    layer: CivilianUnitLayer,
}

#[derive(Bundle)]
pub struct LandMilitaryUnitTileBundle {
    pub tile_bundle: TileBundle,
    pub unit_id: UnitId,
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
    pub network_id: NetworkId,
    pub position: TilePos,
    pub unit_type: UnitType,
    pub civ: Civilization,
}

#[derive(Debug, Event)]
pub struct UnitSelected {
    pub entity: Entity,
    pub position: TilePos,
}

#[derive(Copy, Clone, Debug, Deserialize, Event, Serialize)]
pub struct UnitMoved {
    pub network_id: NetworkId,
    pub from_pos: TilePos,
    pub to_pos: TilePos,
    pub movement_cost: NotNan<f64>,
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
        network_id: NetworkId,
        position: TilePos,
        unit_type: UnitType,
        civ: Civilization,
        movement_points: MovementPoints,
        full_movement_points: FullMovementPoints,
        unit_state: UnitState,
    ) -> Self {
        Self {
            network_id,
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
        network_id: NetworkId,
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
                network_id,
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
        network_id: NetworkId,
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
                network_id,
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
    pub fn new(tile_pos: TilePos, tilemap_id: TilemapId, unit_id: UnitId) -> Self {
        Self {
            tile_bundle: TileBundle {
                position: tile_pos,
                tilemap_id,
                texture_index: TileTextureIndex(UnitSelection::Active.into()),
                ..Default::default()
            },
            unit_id,
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
        unit_id: UnitId,
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
            unit_id,
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
        unit_id: UnitId,
    ) -> Self {
        Self {
            tile_bundle: TileBundle {
                position: tile_pos,
                tilemap_id,
                texture_index: TileTextureIndex(civilian_unit.into()),
                color: TileColor(civ.colors()[1].into()),
                ..Default::default()
            },
            unit_id,
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
        unit_id: UnitId,
    ) -> Self {
        Self {
            tile_bundle: TileBundle {
                position: tile_pos,
                tilemap_id,
                texture_index: TileTextureIndex(land_military_unit.into()),
                color: TileColor(civ.colors()[1].into()),
                ..Default::default()
            },
            unit_id,
            layer: LandMilitaryUnitLayer,
        }
    }
}

/// Handles [`UnitSpawned`] events.
pub fn handle_unit_spawned(
    mut commands: Commands,
    mut network_entity_map: ResMut<NetworkEntityMap>,
    mut unit_state_tilemap_query: Query<(Entity, &mut TileStorage), UnitStateLayerFilter>,
    mut civilian_unit_tilemap_query: Query<(Entity, &mut TileStorage), CivilianUnitLayerFilter>,
    mut land_military_unit_tilemap_query: Query<
        (Entity, &mut TileStorage),
        LandMilitaryUnitLayerFilter,
    >,
    mut unit_spawned_events: EventReader<UnitSpawned>,
) {
    let (unit_state_tilemap_entity, mut unit_state_tile_storage) =
        unit_state_tilemap_query.get_single_mut().unwrap();
    let (civilian_unit_tilemap_entity, mut civilian_unit_tile_storage) =
        civilian_unit_tilemap_query.get_single_mut().unwrap();
    let (land_military_unit_tilemap_entity, mut land_military_unit_tile_storage) =
        land_military_unit_tilemap_query.get_single_mut().unwrap();

    for unit_spawned in unit_spawned_events.read() {
        debug!(?unit_spawned, "handling unit spawned");
        let UnitSpawned {
            network_id,
            position,
            unit_type,
            civ,
        } = unit_spawned;

        match *unit_type {
            UnitType::Civilian(civilian_unit) => {
                let unit_entity = commands
                    .spawn(CivilianUnitBundle::new(
                        *network_id,
                        *position,
                        *civ,
                        civilian_unit,
                    ))
                    .id();
                network_entity_map.0.insert(*network_id, unit_entity);
                let tile_entity = commands
                    .spawn(CivilianUnitTileBundle::new(
                        *position,
                        civilian_unit,
                        *civ,
                        TilemapId(civilian_unit_tilemap_entity),
                        UnitId(unit_entity),
                    ))
                    .id();
                civilian_unit_tile_storage.set(position, tile_entity);
                let tile_entity = commands
                    .spawn(UnitStateTileBundle::new(
                        *position,
                        civilian_unit.into(),
                        *civ,
                        TilemapId(unit_state_tilemap_entity),
                        UnitId(unit_entity),
                    ))
                    .id();
                unit_state_tile_storage.set(position, tile_entity);
            },
            UnitType::LandMilitary(land_military_unit) => {
                let unit_entity = commands
                    .spawn(LandMilitaryUnitBundle::new(
                        *network_id,
                        *position,
                        *civ,
                        land_military_unit,
                    ))
                    .id();
                network_entity_map.0.insert(*network_id, unit_entity);
                let tile_entity = commands
                    .spawn(LandMilitaryUnitTileBundle::new(
                        *position,
                        land_military_unit,
                        *civ,
                        TilemapId(land_military_unit_tilemap_entity),
                        UnitId(unit_entity),
                    ))
                    .id();
                land_military_unit_tile_storage.set(position, tile_entity);
                let tile_entity = commands
                    .spawn(UnitStateTileBundle::new(
                        *position,
                        land_military_unit.into(),
                        *civ,
                        TilemapId(unit_state_tilemap_entity),
                        UnitId(unit_entity),
                    ))
                    .id();
                unit_state_tile_storage.set(position, tile_entity);
            },
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
        (Entity, &mut TilePos, &TileTextureIndex, &mut UnitId),
        UnitSelectionLayerFilter,
    >,
    mut unit_state_tile_query: Query<(&mut TileTextureIndex, &mut UnitId), UnitStateLayerFilter>,
    mut unit_tile_query: Query<(&mut TileTextureIndex, &mut UnitId), UnitLayersFilter>,
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
    for unit_selected in unit_selected_events.read() {
        debug!(?unit_selected, "handling unit selected");
        let UnitSelected {
            entity: selected_unit_entity,
            position: selected_unit_tile_pos,
        } = unit_selected;

        let (&unit_type, &civ, &unit_state) = unit_query.get(*selected_unit_entity).unwrap();
        let mut unit_actions_msg = "".to_owned();

        // Update unit selection tile.
        let active_unit_selection = unit_selection_tile_query.iter_mut().find(
            |(_tile_entity, _tile_pos, &tile_texture, _unit_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            },
        );
        new_unit_selection_tile_bundle = None;
        if let Some((tile_entity, mut tile_pos, _tile_texture, mut unit_id)) = active_unit_selection
        {
            // Update unit selection tile.

            unit_selection_tile_storage.remove(&tile_pos);
            tile_pos.set_if_neq(*selected_unit_tile_pos);
            unit_selection_tile_storage.set(selected_unit_tile_pos, tile_entity);
            unit_id.set_if_neq(UnitId(*selected_unit_entity));
        } else {
            // We need to spawn a new unit selection tile, since there was no currently
            // active unit.

            new_unit_selection_tile_bundle = Some(UnitSelectionTileBundle::new(
                *selected_unit_tile_pos,
                TilemapId(unit_selection_tilemap_entity),
                UnitId(*selected_unit_entity),
            ));
        }

        // Update unit state tile.
        // This should always exist so long as there is a unit at this tile position,
        // even in cases of unit stacking.
        {
            let (mut tile_texture, mut unit_id) = unit_state_tile_storage
                .get(selected_unit_tile_pos)
                .map(|tile_entity| unit_state_tile_query.get_mut(tile_entity).unwrap())
                .expect("selected unit tile position should have unit state tile");
            tile_texture.set_if_neq(TileTextureIndex(unit_state.into()));
            unit_id.set_if_neq(UnitId(*selected_unit_entity));
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
        new_unit_tile_bundles.remove(selected_unit_tile_pos);
        match unit_type {
            UnitType::Civilian(civilian_unit) => {
                let tile_storage = unit_tile_storages
                    .remove(&TypeId::of::<CivilianUnit>())
                    .unwrap();
                update_civilian_unit_tile(
                    selected_unit_tile_pos,
                    civilian_unit,
                    civ,
                    TilemapId(civilian_unit_tilemap_entity),
                    UnitId(*selected_unit_entity),
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
                    selected_unit_tile_pos,
                    land_military_unit,
                    civ,
                    TilemapId(land_military_unit_tilemap_entity),
                    UnitId(*selected_unit_entity),
                    &tile_storage,
                    &mut unit_tile_query,
                    &mut new_unit_tile_bundles,
                );
                if *turn_state.get() == TurnState::Playing {
                    unit_actions_msg += "[F] Fortify\n";
                }
            },
        }
        // Remove other unit tiles at the same tile position.
        for mut tile_storage in unit_tile_storages.into_values() {
            if let Some(tile_entity) = tile_storage.get(selected_unit_tile_pos) {
                commands.entity(tile_entity).despawn();
                tile_storage.remove(selected_unit_tile_pos);
            }
        }

        if *turn_state.get() == TurnState::Playing {
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
pub fn handle_unit_moved(
    mut commands: Commands,
    network_entity_map: Res<NetworkEntityMap>,
    mut unit_selection_tilemap_query: Query<(&mut TileStorage,), UnitSelectionLayerFilter>,
    mut unit_state_tilemap_query: Query<(Entity, &mut TileStorage), UnitStateLayerFilter>,
    mut civilian_unit_tilemap_query: Query<(Entity, &mut TileStorage), CivilianUnitLayerFilter>,
    mut land_military_unit_tilemap_query: Query<
        (Entity, &mut TileStorage),
        LandMilitaryUnitLayerFilter,
    >,
    mut unit_selection_tile_query: Query<
        (Entity, &mut TilePos, &TileTextureIndex, &UnitId),
        UnitSelectionLayerFilter,
    >,
    mut unit_state_tile_query: Query<(&mut TileTextureIndex, &mut UnitId), UnitStateLayerFilter>,
    mut unit_tile_query: Query<(&mut TileTextureIndex, &mut UnitId), UnitLayersFilter>,
    mut unit_query: Query<
        (
            Entity,
            &mut TilePos,
            &UnitType,
            &Civilization,
            &mut MovementPoints,
            &mut UnitState,
        ),
        UnitFilter,
    >,
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
    for unit_moved in unit_moved_events.read() {
        debug!(?unit_moved, "handling unit moved");
        let UnitMoved {
            network_id: unit_network_id,
            from_pos,
            to_pos,
            movement_cost,
        } = unit_moved;
        let moved_unit_entity = network_entity_map
            .0
            .get(unit_network_id)
            .expect("network id of the unit being moved should refer to a known existing entity");

        // Update unit.
        {
            let (_unit_entity, mut tile_pos, _unit_type, _civ, mut movement_points, mut unit_state) =
                unit_query.get_mut(*moved_unit_entity).unwrap();
            assert!(&*tile_pos == from_pos);
            *tile_pos = *to_pos;
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
            new_unit_tile_bundles.remove(tile_pos);
            new_unit_state_tile_bundles.remove(tile_pos);

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
            let Some((unit_entity, _tile_pos, &unit_type, civ, _movement_points, unit_state)) =
                unit_entity
                    .map(|&unit_entity| unit_query.get(unit_entity).unwrap())
                    .or_else(|| {
                        unit_query.iter().find(
                            |(
                                _unit_entity,
                                &unit_tile_pos,
                                _unit_type,
                                _civ,
                                _movement_points,
                                _unit_state,
                            )| { unit_tile_pos == *tile_pos },
                        )
                    })
            else {
                // Remove unit tiles and unit state tile, as there are no units at this tile
                // position.
                for mut tile_storage in unit_tile_storages.into_values() {
                    if let Some(tile_entity) = tile_storage.get(tile_pos) {
                        commands.entity(tile_entity).despawn();
                        tile_storage.remove(tile_pos);
                    }
                }
                if let Some(tile_entity) = unit_state_tile_storage.get(tile_pos) {
                    commands.entity(tile_entity).despawn();
                    unit_state_tile_storage.remove(tile_pos);
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
                        tile_pos,
                        civilian_unit,
                        *civ,
                        TilemapId(civilian_unit_tilemap_entity),
                        UnitId(unit_entity),
                        &tile_storage,
                        &mut unit_tile_query,
                        &mut new_unit_tile_bundles,
                    );

                    // Update unit state tile.
                    if let Some(tile_entity) = unit_state_tile_storage.get(tile_pos) {
                        let (mut tile_texture, mut unit_id) =
                            unit_state_tile_query.get_mut(tile_entity).unwrap();
                        tile_texture.set_if_neq(TileTextureIndex((*unit_state).into()));
                        *unit_id = UnitId(unit_entity);
                    } else {
                        let mut unit_state_tile_bundle = UnitStateTileBundle::new(
                            *tile_pos,
                            UnitType::Civilian(civilian_unit),
                            *civ,
                            TilemapId(unit_state_tilemap_entity),
                            UnitId(unit_entity),
                        );
                        unit_state_tile_bundle.tile_bundle.texture_index =
                            TileTextureIndex((*unit_state).into());
                        new_unit_state_tile_bundles.insert(*tile_pos, unit_state_tile_bundle);
                    }
                },
                UnitType::LandMilitary(land_military_unit) => {
                    // Update unit tile.
                    let tile_storage = unit_tile_storages
                        .remove(&TypeId::of::<LandMilitaryUnit>())
                        .unwrap();
                    update_land_military_unit_tile(
                        tile_pos,
                        land_military_unit,
                        *civ,
                        TilemapId(land_military_unit_tilemap_entity),
                        UnitId(unit_entity),
                        &tile_storage,
                        &mut unit_tile_query,
                        &mut new_unit_tile_bundles,
                    );

                    // Update unit state tile.
                    if let Some(tile_entity) = unit_state_tile_storage.get(tile_pos) {
                        let (mut tile_texture, mut unit_id) =
                            unit_state_tile_query.get_mut(tile_entity).unwrap();
                        tile_texture.set_if_neq(TileTextureIndex((*unit_state).into()));
                        *unit_id = UnitId(unit_entity);
                    } else {
                        let mut unit_state_tile_bundle = UnitStateTileBundle::new(
                            *tile_pos,
                            UnitType::LandMilitary(land_military_unit),
                            *civ,
                            TilemapId(unit_state_tilemap_entity),
                            UnitId(unit_entity),
                        );
                        unit_state_tile_bundle.tile_bundle.texture_index =
                            TileTextureIndex((*unit_state).into());
                        new_unit_state_tile_bundles.insert(*tile_pos, unit_state_tile_bundle);
                    }
                },
            }
            // Remove other unit tiles at the same tile position.
            for mut tile_storage in unit_tile_storages.into_values() {
                if let Some(tile_entity) = tile_storage.get(tile_pos) {
                    commands.entity(tile_entity).despawn();
                    tile_storage.remove(tile_pos);
                }
            }
        }

        // Update unit selection tile.
        let active_unit_selection =
        unit_selection_tile_query
            .iter_mut()
            .find(|(_tile_entity, _tile_pos, &tile_texture, _unit_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });
        if let Some((tile_entity, mut tile_pos, _tile_texture, _unit_id)) = active_unit_selection
            .filter(
                |(_tile_entity, _tile_pos, _tile_texture, UnitId(unit_entity))| {
                    unit_entity == moved_unit_entity
                },
            )
        {
            assert!(&*tile_pos == from_pos);
            unit_selection_tile_storage.remove(from_pos);
            *tile_pos = *to_pos;
            unit_selection_tile_storage.set(to_pos, tile_entity);
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
    civilian_unit_id: UnitId,
    civilian_unit_tile_storage: &TileStorage,
    unit_tile_query: &mut Query<(&mut TileTextureIndex, &mut UnitId), UnitLayersFilter>,
    new_unit_tile_bundles: &mut HashMap<TilePos, UnitTileBundle>,
) {
    if let Some(tile_entity) = civilian_unit_tile_storage.get(tile_pos) {
        let (mut tile_texture, mut unit_id) = unit_tile_query.get_mut(tile_entity).unwrap();
        tile_texture.set_if_neq(TileTextureIndex(civilian_unit.into()));
        unit_id.set_if_neq(civilian_unit_id);
    } else {
        new_unit_tile_bundles.insert(
            *tile_pos,
            UnitTileBundle::Civilian(CivilianUnitTileBundle::new(
                *tile_pos,
                civilian_unit,
                civ,
                civilian_unit_tilemap_id,
                civilian_unit_id,
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
    land_military_unit_id: UnitId,
    land_military_unit_tile_storage: &TileStorage,
    unit_tile_query: &mut Query<(&mut TileTextureIndex, &mut UnitId), UnitLayersFilter>,
    new_unit_tile_bundles: &mut HashMap<TilePos, UnitTileBundle>,
) {
    if let Some(tile_entity) = land_military_unit_tile_storage.get(tile_pos) {
        let (mut tile_texture, mut unit_id) = unit_tile_query.get_mut(tile_entity).unwrap();
        tile_texture.set_if_neq(TileTextureIndex(land_military_unit.into()));
        unit_id.set_if_neq(land_military_unit_id);
    } else {
        new_unit_tile_bundles.insert(
            *tile_pos,
            UnitTileBundle::LandMilitary(LandMilitaryUnitTileBundle::new(
                *tile_pos,
                land_military_unit,
                civ,
                land_military_unit_tilemap_id,
                land_military_unit_id,
            )),
        );
    }
}
