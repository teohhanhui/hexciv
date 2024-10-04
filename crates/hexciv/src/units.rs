use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy_ecs_tilemap::map::TilemapId;
use bevy_ecs_tilemap::tiles::{TileBundle, TileColor, TilePos, TileTextureIndex};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ordered_float::NotNan;

use crate::layers::{
    BaseTerrainLayer, CivilianUnitLayer, LandMilitaryUnitLayer, RiverLayer, TerrainFeaturesLayer,
    UnitSelectionLayer, UnitStateLayer,
};
use crate::types::Civilization;

#[derive(Copy, Clone, Component)]
pub struct Civ(pub Civilization);

#[derive(Copy, Clone, Eq, PartialEq, Component)]
pub enum UnitType {
    Civilian(CivilianUnit),
    LandMilitary(LandMilitaryUnit),
}

#[derive(Copy, Clone, Eq, PartialEq, Component, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum CivilianUnit {
    Settler = 0,
}

#[derive(Copy, Clone, Eq, PartialEq, Component, IntoPrimitive, TryFromPrimitive)]
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

#[derive(Copy, Clone, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum UnitSelection {
    Active = 0,
}

#[derive(Copy, Clone, Eq, PartialEq, Component)]
pub struct UnitId(pub Entity);

#[derive(Bundle)]
pub struct UnitBundle {
    pub position: TilePos,
    pub unit_type: UnitType,
    pub civ: Civ,
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

impl UnitBundle {
    fn new(
        position: TilePos,
        unit_type: UnitType,
        civ: Civilization,
        movement_points: MovementPoints,
        full_movement_points: FullMovementPoints,
        unit_state: UnitState,
    ) -> Self {
        Self {
            position,
            unit_type,
            civ: Civ(civ),
            movement_points,
            full_movement_points,
            unit_state,
        }
    }
}

impl CivilianUnitBundle {
    pub fn new(position: TilePos, civ: Civilization, civilian_unit: CivilianUnit) -> Self {
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
    pub fn new(position: TilePos, civ: Civilization, land_military_unit: LandMilitaryUnit) -> Self {
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
