use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;

#[derive(Component)]
pub struct BaseTerrainLayer;

#[derive(Component)]
pub struct RiverLayer;

#[derive(Component)]
pub struct TerrainFeaturesLayer;

#[derive(Component)]
pub struct UnitSelectionLayer;

#[derive(Component)]
pub struct UnitStateLayer;

#[derive(Component)]
pub struct CivilianUnitLayer;

#[derive(Component)]
pub struct LandMilitaryUnitLayer;

#[derive(QueryFilter)]
pub struct BaseTerrainLayerFilter(
    With<BaseTerrainLayer>,
    Without<RiverLayer>,
    Without<TerrainFeaturesLayer>,
    Without<UnitSelectionLayer>,
    Without<UnitStateLayer>,
    Without<CivilianUnitLayer>,
    Without<LandMilitaryUnitLayer>,
);

#[derive(QueryFilter)]
pub struct RiverLayerFilter(
    With<RiverLayer>,
    Without<BaseTerrainLayer>,
    Without<TerrainFeaturesLayer>,
    Without<UnitSelectionLayer>,
    Without<UnitStateLayer>,
    Without<CivilianUnitLayer>,
    Without<LandMilitaryUnitLayer>,
);

#[derive(QueryFilter)]
pub struct TerrainFeaturesLayerFilter(
    With<TerrainFeaturesLayer>,
    Without<BaseTerrainLayer>,
    Without<RiverLayer>,
    Without<UnitSelectionLayer>,
    Without<UnitStateLayer>,
    Without<CivilianUnitLayer>,
    Without<LandMilitaryUnitLayer>,
);

#[derive(QueryFilter)]
pub struct UnitSelectionLayerFilter(
    With<UnitSelectionLayer>,
    Without<BaseTerrainLayer>,
    Without<RiverLayer>,
    Without<TerrainFeaturesLayer>,
    Without<UnitStateLayer>,
    Without<CivilianUnitLayer>,
    Without<LandMilitaryUnitLayer>,
);

#[derive(QueryFilter)]
pub struct UnitStateLayerFilter(
    With<UnitStateLayer>,
    Without<BaseTerrainLayer>,
    Without<RiverLayer>,
    Without<TerrainFeaturesLayer>,
    Without<UnitSelectionLayer>,
    Without<CivilianUnitLayer>,
    Without<LandMilitaryUnitLayer>,
);

#[derive(QueryFilter)]
pub struct CivilianUnitLayerFilter(
    With<CivilianUnitLayer>,
    Without<BaseTerrainLayer>,
    Without<RiverLayer>,
    Without<TerrainFeaturesLayer>,
    Without<UnitSelectionLayer>,
    Without<UnitStateLayer>,
    Without<LandMilitaryUnitLayer>,
);

#[derive(QueryFilter)]
pub struct LandMilitaryUnitLayerFilter(
    With<LandMilitaryUnitLayer>,
    Without<BaseTerrainLayer>,
    Without<RiverLayer>,
    Without<TerrainFeaturesLayer>,
    Without<UnitSelectionLayer>,
    Without<UnitStateLayer>,
    Without<CivilianUnitLayer>,
);

#[derive(QueryFilter)]
pub struct UnitLayersFilter(
    Or<(With<CivilianUnitLayer>, With<LandMilitaryUnitLayer>)>,
    Without<BaseTerrainLayer>,
    Without<RiverLayer>,
    Without<TerrainFeaturesLayer>,
    Without<UnitSelectionLayer>,
    Without<UnitStateLayer>,
);

pub trait LayerZIndex {
    const Z_INDEX: f32;
}

impl LayerZIndex for BaseTerrainLayer {
    const Z_INDEX: f32 = 0.0;
}

impl LayerZIndex for RiverLayer {
    const Z_INDEX: f32 = 1.0;
}

impl LayerZIndex for TerrainFeaturesLayer {
    const Z_INDEX: f32 = 2.0;
}

impl LayerZIndex for UnitSelectionLayer {
    const Z_INDEX: f32 = 4.0;
}

impl LayerZIndex for UnitStateLayer {
    const Z_INDEX: f32 = 5.0;
}

impl LayerZIndex for CivilianUnitLayer {
    const Z_INDEX: f32 = 6.0;
}

impl LayerZIndex for LandMilitaryUnitLayer {
    const Z_INDEX: f32 = 6.0;
}
