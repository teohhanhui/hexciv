use std::collections::{BTreeMap, HashMap};
use std::ops::Add;

use bevy::prelude::*;
use bevy_ecs_tilemap::helpers::hex_grid::neighbors::{HexNeighbors, HEX_DIRECTIONS};
use bevy_ecs_tilemap::prelude::*;
use bevy_pancam::{DirectionKeys, PanCam};
use bitvec::prelude::*;
use fastlem_random_terrain::{generate_terrain, Site2D, Terrain2D};
use fastrand_contrib::RngExt as _;
use itertools::{chain, repeat_n, Itertools as _};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ordered_float::NotNan;

use crate::game_setup::MapRng;
use crate::layer::{
    BaseTerrainLayer, BaseTerrainLayerFilter, CivilianUnitLayer, LandMilitaryUnitLayer,
    LayerZIndex as _, RiverLayer, RiverLayerFilter, TerrainFeaturesLayer,
    TerrainFeaturesLayerFilter, UnitSelectionLayer, UnitStateLayer,
};
use crate::unit::ActionsLegend;

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

const MAP_TYPE: TilemapType = TilemapType::Hexagon(HexCoordSystem::RowOdd);

const ODD_ROW_OFFSET: f64 = 0.5 * GRID_SIZE.x as f64;

/// The center-to-center distance between adjacent columns of tiles.
const CENTER_TO_CENTER_X: f64 = GRID_SIZE.x as f64;
/// The center-to-center distance between adjacent rows of tiles.
const CENTER_TO_CENTER_Y: f64 = 0.75 * GRID_SIZE.y as f64;

const BOUND_WIDTH: f64 =
    ((MAP_SIDE_LENGTH_X - 1) as f64 * CENTER_TO_CENTER_X + GRID_SIZE.x as f64 + ODD_ROW_OFFSET)
        / 100.0;
const BOUND_HEIGHT: f64 =
    ((MAP_SIDE_LENGTH_Y - 1) as f64 * CENTER_TO_CENTER_Y + GRID_SIZE.y as f64) / 100.0;

const BOUND_MIN: Site2D = Site2D {
    x: -BOUND_WIDTH / 2.0,
    y: -BOUND_HEIGHT / 2.0,
};
const BOUND_MAX: Site2D = Site2D {
    x: BOUND_WIDTH / 2.0,
    y: BOUND_HEIGHT / 2.0,
};
const BOUND_RANGE: Site2D = Site2D {
    x: BOUND_WIDTH,
    y: BOUND_HEIGHT,
};

const VERTEX_OFFSETS: [(f32, f32); 6] = [
    (GRID_SIZE.x * 0.5, -GRID_SIZE.y * 0.25),
    (GRID_SIZE.x * 0.5, GRID_SIZE.y * 0.25),
    (0.0, GRID_SIZE.y * 0.5),
    (-GRID_SIZE.x * 0.5, GRID_SIZE.y * 0.25),
    (-GRID_SIZE.x * 0.5, -GRID_SIZE.y * 0.25),
    (0.0, -GRID_SIZE.y * 0.5),
];

const EXTENDED_VERTEX_OFFSETS: [(f32, f32); 6] = [
    (GRID_SIZE.x, -GRID_SIZE.y * 0.5),
    (GRID_SIZE.x, GRID_SIZE.y * 0.5),
    (0.0, GRID_SIZE.y),
    (-GRID_SIZE.x, GRID_SIZE.y * 0.5),
    (-GRID_SIZE.x, -GRID_SIZE.y * 0.5),
    (0.0, -GRID_SIZE.y),
];

const FRIGID_ZONE_TERRAIN_CHOICES: [BaseTerrain; 2] = [BaseTerrain::Tundra, BaseTerrain::Snow];
const TEMPERATE_ZONE_TERRAIN_CHOICES: [BaseTerrain; 4] = [
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Grassland,
];
const SUBTROPICS_TERRAIN_CHOICES: [BaseTerrain; 7] = [
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Grassland,
    BaseTerrain::Grassland,
    BaseTerrain::Desert,
    BaseTerrain::Desert,
];
const TROPICS_TERRAIN_CHOICES: [BaseTerrain; 7] = [
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Grassland,
    BaseTerrain::Grassland,
    BaseTerrain::Grassland,
    BaseTerrain::Grassland,
    BaseTerrain::Desert,
];

const WOODS_CHOICES: [bool; 5] = [true, false, false, false, false];
const TEMPERATE_RAINFOREST_CHOICES: [bool; 5] = [true, false, false, false, false];
const SUBTROPICAL_RAINFOREST_CHOICES: [bool; 10] = [
    true, false, false, false, false, false, false, false, false, false,
];
const TROPICAL_RAINFOREST_CHOICES: [bool; 3] = [true, false, false];
const OASIS_CHOICES: [bool; 5] = [true, false, false, false, false];
const ICE_CHOICES: [bool; 4] = [true, true, true, false];

#[derive(Resource)]
pub struct MapTerrain(Terrain2D);

#[derive(Copy, Clone, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum BaseTerrain {
    Plains = 0,
    Grassland = 1,
    Desert = 2,
    Tundra = 3,
    Snow = 4,
    PlainsHills = 5,
    GrasslandHills = 6,
    DesertHills = 7,
    TundraHills = 8,
    SnowHills = 9,
    PlainsMountains = 10,
    GrasslandMountains = 11,
    DesertMountains = 12,
    TundraMountains = 13,
    SnowMountains = 14,
    Coast = 15,
    Ocean = 16,
}

#[derive(Copy, Clone, IntoPrimitive)]
#[repr(u32)]
pub enum BaseTerrainVariant {
    Hills = 5,
    Mountains = 10,
}

pub type RiverEdges = BitArr!(for 6, in u32, Lsb0);

#[derive(Copy, Clone, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum TerrainFeatures {
    Woods = 0,
    Rainforest = 1,
    Marsh = 2,
    Floodplains = 3,
    Oasis = 4,
    Cliffs = 5,
    Ice = 6,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct SpawnTilemapSet;

enum EarthLatitude {
    ArticCirle,
    TropicOfCancer,
    TropicOfCapricorn,
    AntarcticCircle,
}

impl BaseTerrain {
    pub const HILLS: [Self; 5] = [
        Self::PlainsHills,
        Self::GrasslandHills,
        Self::DesertHills,
        Self::TundraHills,
        Self::SnowHills,
    ];
    pub const MOUNTAINS: [Self; 5] = [
        Self::PlainsMountains,
        Self::GrasslandMountains,
        Self::DesertMountains,
        Self::TundraMountains,
        Self::SnowMountains,
    ];

    pub fn is_hills(&self) -> bool {
        Self::HILLS.contains(self)
    }

    pub fn is_mountains(&self) -> bool {
        Self::MOUNTAINS.contains(self)
    }
}

impl Add<BaseTerrainVariant> for BaseTerrain {
    type Output = Self;

    fn add(self, rhs: BaseTerrainVariant) -> Self::Output {
        match self {
            Self::Plains | Self::Grassland | Self::Desert | Self::Tundra | Self::Snow => {
                let base: u32 = self.into();
                let variant: u32 = rhs.into();
                Self::try_from(base + variant).unwrap()
            },
            Self::PlainsHills
            | Self::GrasslandHills
            | Self::DesertHills
            | Self::TundraHills
            | Self::SnowHills
            | Self::PlainsMountains
            | Self::GrasslandMountains
            | Self::DesertMountains
            | Self::TundraMountains
            | Self::SnowMountains => {
                unimplemented!("base terrain variants are not stackable");
            },
            Self::Coast | Self::Ocean => {
                unimplemented!("coast and ocean base terrain do not have variants");
            },
        }
    }
}

impl EarthLatitude {
    pub const fn latitude(&self) -> f64 {
        match self {
            Self::ArticCirle => 66.57,
            Self::TropicOfCancer => 23.43,
            Self::TropicOfCapricorn => -23.43,
            Self::AntarcticCircle => -66.57,
        }
    }
}

/// Generates the initial tilemap.
pub fn spawn_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut map_rng: ResMut<MapRng>,
    mut actions_legend_text_query: Query<(&mut Text,), With<ActionsLegend>>,
) {
    let rng = &mut map_rng.0;
    info!(seed = rng.get_seed(), "map seed");

    let (mut actions_legend_text,) = actions_legend_text_query.get_single_mut().unwrap();

    actions_legend_text.sections[0].value = "".to_owned();

    let map_size = TilemapSize {
        x: MAP_SIDE_LENGTH_X,
        y: MAP_SIDE_LENGTH_Y,
    };

    let terrain = {
        let config = fastlem_random_terrain::Config {
            seed: rng.u32(..),
            land_ratio: rng.f64_range(0.29..=0.6),
            ..Default::default()
        };
        info!(?config, "fastlem-random-terrain config");
        generate_terrain(&config, BOUND_MIN, BOUND_MAX, BOUND_RANGE)
    };

    // Spawn base terrain layer.

    let base_terrain_image_handles = vec![
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
    let base_terrain_texture_vec = TilemapTexture::Vector(base_terrain_image_handles);

    let mut base_terrain_tile_storage = TileStorage::empty(map_size);
    let base_terrain_tilemap_entity = commands.spawn_empty().id();

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let elevations = VERTEX_OFFSETS.map(|(vertex_offset_x, vertex_offset_y)| {
                let x = BOUND_MIN.x
                    + (f64::from(GRID_SIZE.x) / 2.0
                        + f64::from(tile_pos.x) * CENTER_TO_CENTER_X
                        + if tile_pos.y % 2 == 0 {
                            0.0
                        } else {
                            ODD_ROW_OFFSET
                        }
                        + f64::from(vertex_offset_x))
                        / 100.0;
                let y = BOUND_MIN.y
                    + (f64::from(GRID_SIZE.y) / 2.0
                        + f64::from(map_size.y - tile_pos.y - 1) * CENTER_TO_CENTER_Y
                        + f64::from(vertex_offset_y))
                        / 100.0;
                let site = Site2D { x, y };
                terrain.get_elevation(&site)
            });
            let elevations: Vec<_> = elevations
                .into_iter()
                .flat_map(|elevation| {
                    elevation
                        .filter(|elevation| !elevation.is_nan())
                        .map(|elevation| NotNan::new(elevation).unwrap())
                })
                .collect();
            let elevation = elevations.iter().sum::<NotNan<_>>()
                / NotNan::from(
                    u8::try_from(elevations.len()).expect("`elevations.len()` should fit in `u8`"),
                );
            let texture_index = if elevation < NotNan::new(0.05).unwrap() {
                TileTextureIndex(BaseTerrain::Ocean.into())
            } else {
                let latitude = NotNan::new(-90.0).unwrap()
                    + NotNan::new(180.0).unwrap()
                        * ((NotNan::from(tile_pos.y) + NotNan::new(0.5).unwrap())
                            / NotNan::from(map_size.y));

                let base_terrain = choose_base_terrain_by_latitude(rng, latitude);

                TileTextureIndex(if elevation >= NotNan::new(25.0).unwrap() {
                    (base_terrain + BaseTerrainVariant::Mountains).into()
                } else if elevation >= NotNan::new(5.0).unwrap() {
                    (base_terrain + BaseTerrainVariant::Hills).into()
                } else {
                    base_terrain.into()
                })
            };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    tilemap_id: TilemapId(base_terrain_tilemap_entity),
                    texture_index,
                    ..Default::default()
                })
                .insert(BaseTerrainLayer)
                .id();
            base_terrain_tile_storage.set(&tile_pos, tile_entity);
        }
    }

    commands
        .entity(base_terrain_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: base_terrain_tile_storage,
            texture: base_terrain_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(
                &map_size,
                &GRID_SIZE,
                &MAP_TYPE,
                BaseTerrainLayer::Z_INDEX,
            ),
            ..Default::default()
        })
        .insert(BaseTerrainLayer);

    // Spawn river layer.

    let river_image_handles = {
        let image_map: BTreeMap<u32, Handle<Image>> = repeat_n([true, false].into_iter(), 6)
            .multi_cartesian_product()
            .map(|data| {
                let mut bits: RiverEdges = BitArray::<_>::ZERO;
                for (i, &v) in data.iter().enumerate() {
                    bits.set(i, v);
                }
                (
                    bits.load(),
                    asset_server.load(format!(
                        "tiles/river-{edges}.png",
                        edges = data
                            .iter()
                            .enumerate()
                            .map(|(i, &v)| if v { i.to_string() } else { "x".to_owned() })
                            .join("")
                    )),
                )
            })
            .collect();
        let size = usize::try_from(*image_map.last_key_value().unwrap().0).unwrap() + 1;
        let mut image_vec = vec![asset_server.load("tiles/transparent.png"); size];
        for (key, image) in image_map {
            image_vec[usize::try_from(key).unwrap()] = image;
        }
        image_vec
    };
    let river_texture_vec = TilemapTexture::Vector(river_image_handles);

    let river_tile_storage = TileStorage::empty(map_size);
    let river_tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(river_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: river_tile_storage,
            texture: river_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(
                &map_size,
                &GRID_SIZE,
                &MAP_TYPE,
                RiverLayer::Z_INDEX,
            ),
            ..Default::default()
        })
        .insert(RiverLayer);

    // Spawn terrain features layer.

    let terrain_features_image_handles = vec![
        asset_server.load("tiles/woods.png"),
        asset_server.load("tiles/rainforest.png"),
        // TODO: marsh
        asset_server.load("tiles/transparent.png"),
        // TODO: floodplains
        asset_server.load("tiles/transparent.png"),
        asset_server.load("tiles/oasis.png"),
        // TODO: cliffs
        asset_server.load("tiles/transparent.png"),
        asset_server.load("tiles/ice.png"),
    ];
    let terrain_features_texture_vec = TilemapTexture::Vector(terrain_features_image_handles);

    let terrain_features_tile_storage = TileStorage::empty(map_size);
    let terrain_features_tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(terrain_features_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: terrain_features_tile_storage,
            texture: terrain_features_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(
                &map_size,
                &GRID_SIZE,
                &MAP_TYPE,
                TerrainFeaturesLayer::Z_INDEX,
            ),
            ..Default::default()
        })
        .insert(TerrainFeaturesLayer);

    commands.insert_resource(MapTerrain(terrain));

    // Spawn unit selection layer.

    let unit_selection_image_handles = vec![asset_server.load("units/active.png")];
    let unit_selection_texture_vec = TilemapTexture::Vector(unit_selection_image_handles);

    let unit_selection_tile_storage = TileStorage::empty(map_size);
    let unit_selection_tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(unit_selection_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: unit_selection_tile_storage,
            texture: unit_selection_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(
                &map_size,
                &GRID_SIZE,
                &MAP_TYPE,
                UnitSelectionLayer::Z_INDEX,
            ),
            ..Default::default()
        })
        .insert(UnitSelectionLayer);

    // Spawn unit state layer.

    let unit_state_image_handles = vec![
        asset_server.load("units/civilian-ready.png"),
        asset_server.load("units/land-military-ready.png"),
        asset_server.load("units/land-military-fortified.png"),
        asset_server.load("units/civilian-ready-out-of-orders.png"),
        asset_server.load("units/land-military-ready-out-of-orders.png"),
        asset_server.load("units/land-military-fortified-out-of-orders.png"),
        asset_server.load("units/civilian-out-of-moves.png"),
        asset_server.load("units/land-military-out-of-moves.png"),
    ];
    let unit_state_texture_vec = TilemapTexture::Vector(unit_state_image_handles);

    let unit_state_tile_storage = TileStorage::empty(map_size);
    let unit_state_tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(unit_state_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: unit_state_tile_storage,
            texture: unit_state_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(
                &map_size,
                &GRID_SIZE,
                &MAP_TYPE,
                UnitStateLayer::Z_INDEX,
            ),
            ..Default::default()
        })
        .insert(UnitStateLayer);

    // Spawn civilian unit layer.

    let civilian_unit_image_handles = vec![asset_server.load("units/settler.png")];
    let civilian_unit_texture_vec = TilemapTexture::Vector(civilian_unit_image_handles);

    let civilian_unit_tile_storage = TileStorage::empty(map_size);
    let civilian_unit_tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(civilian_unit_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: civilian_unit_tile_storage,
            texture: civilian_unit_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(
                &map_size,
                &GRID_SIZE,
                &MAP_TYPE,
                CivilianUnitLayer::Z_INDEX,
            ),
            ..Default::default()
        })
        .insert(CivilianUnitLayer);

    // Spawn land military unit layer.

    let land_military_unit_image_handles = vec![asset_server.load("units/warrior.png")];
    let land_military_unit_texture_vec = TilemapTexture::Vector(land_military_unit_image_handles);

    let land_military_unit_tile_storage = TileStorage::empty(map_size);
    let land_military_unit_tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(land_military_unit_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: land_military_unit_tile_storage,
            texture: land_military_unit_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(
                &map_size,
                &GRID_SIZE,
                &MAP_TYPE,
                LandMilitaryUnitLayer::Z_INDEX,
            ),
            ..Default::default()
        })
        .insert(LandMilitaryUnitLayer);
}

#[allow(clippy::too_many_arguments)]
pub fn post_spawn_tilemap(
    mut commands: Commands,
    mut map_rng: ResMut<MapRng>,
    map_terrain: Res<MapTerrain>,
    base_terrain_tilemap_query: Query<(&TilemapSize, &TileStorage), BaseTerrainLayerFilter>,
    mut river_tilemap_query: Query<(Entity, &mut TileStorage), RiverLayerFilter>,
    mut terrain_features_tilemap_query: Query<
        (Entity, &mut TileStorage),
        TerrainFeaturesLayerFilter,
    >,
    mut base_terrain_tile_query: Query<(&mut TileTextureIndex,), BaseTerrainLayerFilter>,
) {
    let rng = &mut map_rng.0;
    let terrain = &map_terrain.0;

    let (map_size, base_terrain_tile_storage) = base_terrain_tilemap_query.get_single().unwrap();
    let (terrain_features_tilemap_entity, mut terrain_features_tile_storage) =
        terrain_features_tilemap_query.get_single_mut().unwrap();
    let (river_tilemap_entity, mut river_tile_storage) =
        river_tilemap_query.get_single_mut().unwrap();

    let mut river_edges_map: HashMap<TilePos, RiverEdges> = HashMap::new();

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let tile_entity = base_terrain_tile_storage.get(&tile_pos).unwrap();
            let (&tile_texture,) = base_terrain_tile_query.get(tile_entity).unwrap();
            let neighbor_positions =
                HexNeighbors::get_neighboring_positions_row_odd(&tile_pos, map_size);
            let neighbor_entities = neighbor_positions.entities(base_terrain_tile_storage);

            let latitude = NotNan::new(-90.0).unwrap()
                + NotNan::new(180.0).unwrap()
                    * ((NotNan::from(tile_pos.y) + NotNan::new(0.5).unwrap())
                        / NotNan::from(map_size.y));

            if matches!(tile_texture, TileTextureIndex(t) if t == u32::from(BaseTerrain::Ocean))
                && neighbor_entities.iter().any(|neighbor_entity| {
                    let (tile_texture,) = base_terrain_tile_query.get(*neighbor_entity).unwrap();
                    ![BaseTerrain::Ocean.into(), BaseTerrain::Coast.into()]
                        .contains(&tile_texture.0)
                })
            {
                let (mut tile_texture,) = base_terrain_tile_query.get_mut(tile_entity).unwrap();
                tile_texture.0 = BaseTerrain::Coast.into();
            }

            if [
                BaseTerrain::Plains.into(),
                BaseTerrain::PlainsHills.into(),
                BaseTerrain::Grassland.into(),
                BaseTerrain::GrasslandHills.into(),
                BaseTerrain::Tundra.into(),
                BaseTerrain::TundraHills.into(),
            ]
            .contains(&tile_texture.0)
                && rng.choice(WOODS_CHOICES).unwrap()
            {
                let tile_entity = commands
                    .spawn(TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(terrain_features_tilemap_entity),
                        texture_index: TileTextureIndex(TerrainFeatures::Woods.into()),
                        ..Default::default()
                    })
                    .insert(TerrainFeaturesLayer)
                    .id();
                terrain_features_tile_storage.set(&tile_pos, tile_entity);
            } else if [BaseTerrain::Plains.into(), BaseTerrain::PlainsHills.into()]
                .contains(&tile_texture.0)
                && {
                    if *latitude >= EarthLatitude::ArticCirle.latitude()
                        || *latitude <= EarthLatitude::AntarcticCircle.latitude()
                    {
                        false
                    } else if *latitude >= 35.0 || *latitude <= -35.0 {
                        rng.choice(TEMPERATE_RAINFOREST_CHOICES).unwrap()
                    } else if *latitude >= EarthLatitude::TropicOfCancer.latitude()
                        || *latitude <= EarthLatitude::TropicOfCapricorn.latitude()
                    {
                        rng.choice(SUBTROPICAL_RAINFOREST_CHOICES).unwrap()
                    } else {
                        rng.choice(TROPICAL_RAINFOREST_CHOICES).unwrap()
                    }
                }
            {
                let tile_entity = commands
                    .spawn(TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(terrain_features_tilemap_entity),
                        texture_index: TileTextureIndex(TerrainFeatures::Rainforest.into()),
                        ..Default::default()
                    })
                    .insert(TerrainFeaturesLayer)
                    .id();
                terrain_features_tile_storage.set(&tile_pos, tile_entity);
            }

            if matches!(tile_texture, TileTextureIndex(t) if t == u32::from(BaseTerrain::Desert))
                && rng.choice(OASIS_CHOICES).unwrap()
            {
                let tile_entity = commands
                    .spawn(TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(terrain_features_tilemap_entity),
                        texture_index: TileTextureIndex(TerrainFeatures::Oasis.into()),
                        ..Default::default()
                    })
                    .insert(TerrainFeaturesLayer)
                    .id();
                terrain_features_tile_storage.set(&tile_pos, tile_entity);
            }

            if [BaseTerrain::Ocean.into(), BaseTerrain::Coast.into()].contains(&tile_texture.0)
                && (*latitude >= EarthLatitude::ArticCirle.latitude()
                    || *latitude <= EarthLatitude::AntarcticCircle.latitude())
                && rng.choice(ICE_CHOICES).unwrap()
            {
                let tile_entity = commands
                    .spawn(TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(terrain_features_tilemap_entity),
                        texture_index: TileTextureIndex(TerrainFeatures::Ice.into()),
                        ..Default::default()
                    })
                    .insert(TerrainFeaturesLayer)
                    .id();
                terrain_features_tile_storage.set(&tile_pos, tile_entity);
            }

            if ![
                BaseTerrain::Ocean.into(),
                BaseTerrain::Coast.into(),
                // Exclude lowlands and deserts as river source.
                BaseTerrain::Plains.into(),
                BaseTerrain::Grassland.into(),
                BaseTerrain::Desert.into(),
                BaseTerrain::DesertHills.into(),
                BaseTerrain::DesertMountains.into(),
                BaseTerrain::Tundra.into(),
                BaseTerrain::Snow.into(),
            ]
            .contains(&tile_texture.0)
            {
                let mut elevations: Vec<_> = chain(VERTEX_OFFSETS, EXTENDED_VERTEX_OFFSETS)
                    .map(|(vertex_offset_x, vertex_offset_y)| {
                        let x = BOUND_MIN.x
                            + (f64::from(GRID_SIZE.x) / 2.0
                                + f64::from(tile_pos.x) * CENTER_TO_CENTER_X
                                + if tile_pos.y % 2 == 0 {
                                    0.0
                                } else {
                                    ODD_ROW_OFFSET
                                }
                                + f64::from(vertex_offset_x))
                                / 100.0;
                        let y = BOUND_MIN.y
                            + (f64::from(GRID_SIZE.y) / 2.0
                                + f64::from(map_size.y - tile_pos.y - 1) * CENTER_TO_CENTER_Y
                                + f64::from(vertex_offset_y))
                                / 100.0;
                        let site = Site2D { x, y };
                        terrain.get_elevation(&site)
                    })
                    .collect();
                let dest_elevations = elevations.split_off(6);
                let elevations: Vec<_> = elevations
                    .into_iter()
                    .flat_map(|elevation| {
                        elevation
                            .filter(|elevation| !elevation.is_nan())
                            .map(|elevation| NotNan::new(elevation).unwrap())
                    })
                    .collect();
                if elevations.len() < 6 {
                    continue;
                }
                let (vertex_min, elevation_min) = {
                    elevations
                        .into_iter()
                        .enumerate()
                        .reduce(|(vertex_min, elevation_min), (i, elevation)| {
                            let elevation_min = elevation_min.min(elevation);
                            let vertex_min = if elevation_min == elevation {
                                i
                            } else {
                                vertex_min
                            };
                            (vertex_min, elevation_min)
                        })
                        .unwrap()
                };
                let Some(dest_elevation) = dest_elevations[vertex_min] else {
                    continue;
                };
                if dest_elevation.is_nan() {
                    continue;
                }
                let dest_elevation = NotNan::new(dest_elevation).unwrap();
                if elevation_min <= dest_elevation {
                    // Avoid creating river edges going to the same or higher elevation.
                    continue;
                }

                let edge_a = (vertex_min + 5) % 6;
                let edge_b = vertex_min;
                if let Some(edge_adjacent_tile_entity) =
                    neighbor_entities.get(HEX_DIRECTIONS[edge_a])
                {
                    let (tile_texture,) = base_terrain_tile_query
                        .get(*edge_adjacent_tile_entity)
                        .unwrap();
                    if [BaseTerrain::Ocean.into(), BaseTerrain::Coast.into()]
                        .contains(&tile_texture.0)
                    {
                        // Avoid creating river edges parallel to the sea shore / lake shore.
                        continue;
                    }
                }
                if let Some(edge_adjacent_tile_entity) =
                    neighbor_entities.get(HEX_DIRECTIONS[edge_b])
                {
                    let (tile_texture,) = base_terrain_tile_query
                        .get(*edge_adjacent_tile_entity)
                        .unwrap();
                    if [BaseTerrain::Ocean.into(), BaseTerrain::Coast.into()]
                        .contains(&tile_texture.0)
                    {
                        // Avoid creating river edges parallel to the sea shore / lake shore.
                        continue;
                    }
                }

                if let Some(tile_pos) = neighbor_positions.get(HEX_DIRECTIONS[edge_a]) {
                    let river_edges = river_edges_map
                        .entry(*tile_pos)
                        .or_insert(BitArray::<_>::ZERO);
                    let river_edge = (edge_a + 2) % 6;
                    river_edges.set(river_edge, true);
                }
                if let Some(tile_pos) = neighbor_positions.get(HEX_DIRECTIONS[edge_b]) {
                    let river_edges = river_edges_map
                        .entry(*tile_pos)
                        .or_insert(BitArray::<_>::ZERO);
                    let river_edge = (edge_b + 4) % 6;
                    river_edges.set(river_edge, true);
                }
            }
        }
    }

    for (tile_pos, river_edges) in river_edges_map {
        let tile_entity = commands
            .spawn(TileBundle {
                position: tile_pos,
                tilemap_id: TilemapId(river_tilemap_entity),
                texture_index: TileTextureIndex(river_edges.load()),
                ..Default::default()
            })
            .insert(RiverLayer)
            .id();
        river_tile_storage.set(&tile_pos, tile_entity);
    }
}

pub fn upgrade_camera(mut commands: Commands, camera_query: Query<(Entity,), With<Camera2d>>) {
    let (camera_entity,) = camera_query.get_single().unwrap();

    commands.entity(camera_entity).insert(PanCam {
        grab_buttons: vec![MouseButton::Left],
        move_keys: DirectionKeys::arrows_and_wasd(),
        zoom_to_cursor: true,
        min_scale: 1.0,
        max_scale: 8.0,
        min_x: -((MAP_SIDE_LENGTH_X - 1) as f64 * CENTER_TO_CENTER_X
            + GRID_SIZE.x as f64
            + ODD_ROW_OFFSET) as f32,
        max_x: ((MAP_SIDE_LENGTH_X - 1) as f64 * CENTER_TO_CENTER_X
            + GRID_SIZE.x as f64
            + ODD_ROW_OFFSET) as f32,
        min_y: -((MAP_SIDE_LENGTH_Y - 1) as f64 * CENTER_TO_CENTER_Y + GRID_SIZE.y as f64) as f32,
        max_y: ((MAP_SIDE_LENGTH_Y - 1) as f64 * CENTER_TO_CENTER_Y + GRID_SIZE.y as f64) as f32,
        ..Default::default()
    });
}

fn choose_base_terrain_by_latitude(rng: &mut fastrand::Rng, latitude: NotNan<f64>) -> BaseTerrain {
    if *latitude >= EarthLatitude::ArticCirle.latitude()
        || *latitude <= EarthLatitude::AntarcticCircle.latitude()
    {
        rng.choice(FRIGID_ZONE_TERRAIN_CHOICES).unwrap()
    } else if *latitude >= 35.0 || *latitude <= -35.0 {
        rng.choice(TEMPERATE_ZONE_TERRAIN_CHOICES).unwrap()
    } else if *latitude >= EarthLatitude::TropicOfCancer.latitude()
        || *latitude <= EarthLatitude::TropicOfCapricorn.latitude()
    {
        rng.choice(SUBTROPICS_TERRAIN_CHOICES).unwrap()
    } else {
        rng.choice(TROPICS_TERRAIN_CHOICES).unwrap()
    }
}
