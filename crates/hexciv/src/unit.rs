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
    LandMilitaryUnitLayerFilter, RiverLayer, TerrainFeaturesLayer, UnitSelectionLayer,
    UnitStateLayer, UnitStateLayerFilter,
};
use crate::peer::{NetworkEntityMap, NetworkId};

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

#[derive(Debug, Event)]
pub struct UnitMoved {
    pub entity: Entity,
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
