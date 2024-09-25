use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy_ecs_tilemap::map::TilemapId;
use bevy_ecs_tilemap::tiles::{TileBundle, TileColor, TilePos, TileTextureIndex};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ordered_float::NotNan;

use crate::types::Civilization;

#[derive(Copy, Clone, Component)]
pub struct Civ(pub Civilization);

#[derive(Copy, Clone, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum CivilianUnit {
    Settler = 0,
}

#[derive(Copy, Clone, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum LandMilitaryUnit {
    Warrior = 0,
}

#[derive(Copy, Clone, Component)]
pub struct MovementPoints(pub NotNan<f64>);

#[derive(Copy, Clone, Component)]
pub struct FullMovementPoints(pub NotNan<f64>);

#[derive(Bundle)]
pub struct UnitBundle {
    pub tile_bundle: TileBundle,
    pub civ: Civ,
    pub movement_points: MovementPoints,
    pub full_movement_points: FullMovementPoints,
}

#[derive(Bundle)]
pub struct CivilianUnitBundle {
    pub unit_bundle: UnitBundle,
}

#[derive(Bundle)]
pub struct LandMilitaryUnitBundle {
    unit_bundle: UnitBundle,
}

#[derive(Event)]
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
        tilemap_id: TilemapId,
        texture_index: TileTextureIndex,
        civ: Civilization,
        movement_points: MovementPoints,
        full_movement_points: FullMovementPoints,
    ) -> Self {
        Self {
            tile_bundle: TileBundle {
                position,
                tilemap_id,
                texture_index,
                color: TileColor(civ.colors()[1].into()),
                ..Default::default()
            },
            civ: Civ(civ),
            movement_points,
            full_movement_points,
        }
    }
}

impl CivilianUnitBundle {
    pub fn new(
        position: TilePos,
        tilemap_id: TilemapId,
        civ: Civilization,
        civilian_unit: CivilianUnit,
    ) -> Self {
        let texture_index = TileTextureIndex(civilian_unit.into());
        let (movement_points, full_movement_points) = {
            let movement_points = NotNan::from(civilian_unit.movement_points());
            (
                MovementPoints(movement_points),
                FullMovementPoints(movement_points),
            )
        };

        Self {
            unit_bundle: UnitBundle::new(
                position,
                tilemap_id,
                texture_index,
                civ,
                movement_points,
                full_movement_points,
            ),
        }
    }
}

impl LandMilitaryUnitBundle {
    pub fn new(
        position: TilePos,
        tilemap_id: TilemapId,
        civ: Civilization,
        land_military_unit: LandMilitaryUnit,
    ) -> Self {
        let texture_index = TileTextureIndex(land_military_unit.into());
        let (movement_points, full_movement_points) = {
            let movement_points = NotNan::from(land_military_unit.movement_points());
            (
                MovementPoints(movement_points),
                FullMovementPoints(movement_points),
            )
        };

        Self {
            unit_bundle: UnitBundle::new(
                position,
                tilemap_id,
                texture_index,
                civ,
                movement_points,
                full_movement_points,
            ),
        }
    }
}
