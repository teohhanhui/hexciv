use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy_ecs_tilemap::tiles::TilePos;
use ordered_float::NotNan;

use crate::types::Civilization;

#[derive(Copy, Clone, Component)]
pub struct Civ(pub Civilization);

#[derive(Copy, Clone, Component)]
pub struct MovementPoints(pub NotNan<f64>);

#[derive(Copy, Clone, Component)]
pub struct FullMovementPoints(pub NotNan<f64>);

#[derive(Bundle)]
pub struct UnitBundle {
    pub position: TilePos,
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
    pub unit_bundle: UnitBundle,
}

#[derive(Event)]
pub struct UnitMoved {
    pub unit_entity: Entity,
    pub from_pos: TilePos,
    pub to_pos: TilePos,
    pub movement_points: MovementPoints,
}
