use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::iter::zip;
use std::ops::Add;

use bevy::prelude::*;
use bevy_ecs_tilemap::helpers::hex_grid::axial::AxialPos;
use bevy_ecs_tilemap::helpers::hex_grid::neighbors::{HEX_DIRECTIONS, HexNeighbors};
use bevy_ecs_tilemap::prelude::*;
use bevy_pancam::{DirectionKeys, PanCam};
use bitvec::prelude::*;
use derive_more::Display;
use fastlem_random_terrain::{Site2D, Terrain2D, generate_terrain};
use fastrand_contrib::RngExt as _;
use itertools::{Itertools as _, chain, repeat_n};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ordered_float::NotNan;
use strum::VariantArray;

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

/// Offsets of vertices that lie in each [`HexVertexDirection`].
///
/// The offsets are in [`Site2D`] coordinates.
const VERTEX_OFFSETS: [(f32, f32); 6] = [
    (GRID_SIZE.x * 0.5, -GRID_SIZE.y * 0.25),
    (0.0, -GRID_SIZE.y * 0.5),
    (-GRID_SIZE.x * 0.5, -GRID_SIZE.y * 0.25),
    (-GRID_SIZE.x * 0.5, GRID_SIZE.y * 0.25),
    (0.0, GRID_SIZE.y * 0.5),
    (GRID_SIZE.x * 0.5, GRID_SIZE.y * 0.25),
];

/// Offsets to the closest vertex of tiles that lie in each
/// [`HexVertexDirection`].
///
/// The offsets are in [`Site2D`] coordinates.
const EXTENDED_VERTEX_OFFSETS: [(f32, f32); 6] = [
    (GRID_SIZE.x, -GRID_SIZE.y * 0.5),
    (0.0, -GRID_SIZE.y),
    (-GRID_SIZE.x, -GRID_SIZE.y * 0.5),
    (-GRID_SIZE.x, GRID_SIZE.y * 0.5),
    (0.0, GRID_SIZE.y),
    (GRID_SIZE.x, GRID_SIZE.y * 0.5),
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

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Component)]
pub struct River;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Component)]
pub struct RiverEdge {
    pub source: AxialPos,
    pub destination: AxialPos,
    pub stream_order: StreamOrder,
}

/// The direction extending outwards from each vertex of a hexagonal tile.
///
/// `Zero` corresponds with `NorthEast` for row-oriented tiles.
///
/// The vertices are in counter-clockwise order.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, VariantArray)]
pub enum HexVertexDirection {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
}

#[derive(Debug, Display)]
pub enum HexVertexDirectionError {
    #[display("invalid offset")]
    InvalidOffset,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct StreamOrder(pub u8);

pub type RiverHexEdges = BitArr!(for 6, in u32, Lsb0);

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

impl River {
    pub const MIN_LEN: usize = 2;
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

impl TryFrom<(AxialPos, AxialPos)> for HexVertexDirection {
    type Error = HexVertexDirectionError;

    fn try_from((a, b): (AxialPos, AxialPos)) -> Result<Self, Self::Error> {
        let offset_pos = Self::OFFSETS
            .iter()
            .position(|&offset| offset == (b - a))
            .ok_or(Self::Error::InvalidOffset)?;
        Ok(Self::VARIANTS[offset_pos])
    }
}

impl HexVertexDirection {
    /// Offsets of tiles that lie in each [`HexVertexDirection`].
    pub const OFFSETS: [AxialPos; 6] = [
        AxialPos { q: 1, r: 1 },
        AxialPos { q: -1, r: 2 },
        AxialPos { q: -2, r: 1 },
        AxialPos { q: -1, r: -1 },
        AxialPos { q: 1, r: -2 },
        AxialPos { q: 2, r: -1 },
    ];
}

impl Error for HexVertexDirectionError {}

impl Add for StreamOrder {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if self.0 == rhs.0 {
            Self(self.0 + 1)
        } else {
            self.max(rhs)
        }
    }
}

/// Generates the initial tilemap.
pub fn spawn_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut map_rng: ResMut<MapRng>,
    actions_legend_text_query: Single<(&mut Text,), With<ActionsLegend>>,
) {
    let rng = &mut map_rng.0;
    info!(seed = rng.get_seed(), "map seed");

    let (mut actions_legend_text,) = actions_legend_text_query.into_inner();

    actions_legend_text.0 = "".to_owned();

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
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, BaseTerrainLayer::Z_INDEX),
            ..Default::default()
        })
        .insert(BaseTerrainLayer);

    // Spawn river layer.

    let river_image_handles = {
        let image_map: BTreeMap<u32, Handle<Image>> = repeat_n([true, false].into_iter(), 6)
            .multi_cartesian_product()
            .map(|data| {
                let mut bits: RiverHexEdges = BitArray::<_>::ZERO;
                for (i, &v) in data.iter().enumerate() {
                    bits.set(i, v);
                }
                (
                    bits.load(),
                    asset_server.load(format!(
                        "tiles/river/river-{edges}.png",
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
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, RiverLayer::Z_INDEX),
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
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, TerrainFeaturesLayer::Z_INDEX),
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
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, UnitSelectionLayer::Z_INDEX),
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
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, UnitStateLayer::Z_INDEX),
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
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, CivilianUnitLayer::Z_INDEX),
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
            anchor: TilemapAnchor::Center,
            transform: Transform::from_xyz(0.0, 0.0, LandMilitaryUnitLayer::Z_INDEX),
            ..Default::default()
        })
        .insert(LandMilitaryUnitLayer);
}

#[allow(clippy::too_many_arguments)]
pub fn post_spawn_tilemap(
    mut commands: Commands,
    mut map_rng: ResMut<MapRng>,
    map_terrain: Res<MapTerrain>,
    base_terrain_tilemap_query: Single<(&TilemapSize, &TileStorage), BaseTerrainLayerFilter>,
    river_tilemap_query: Single<(Entity, &mut TileStorage), RiverLayerFilter>,
    terrain_features_tilemap_query: Single<(Entity, &mut TileStorage), TerrainFeaturesLayerFilter>,
    mut base_terrain_tile_query: Query<(&mut TileTextureIndex,), BaseTerrainLayerFilter>,
) {
    let rng = &mut map_rng.0;
    let terrain = &map_terrain.0;

    let (map_size, base_terrain_tile_storage) = base_terrain_tilemap_query.into_inner();
    let (terrain_features_tilemap_entity, mut terrain_features_tile_storage) =
        terrain_features_tilemap_query.into_inner();
    let (river_tilemap_entity, mut river_tile_storage) = river_tilemap_query.into_inner();

    let mut river_edges: Vec<RiverEdge> = vec![];

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
                let mut vertex_elevations: Vec<_> = chain(VERTEX_OFFSETS, EXTENDED_VERTEX_OFFSETS)
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
                        terrain
                            .get_elevation(&site)
                            .filter(|elevation| !elevation.is_nan())
                            .map(|elevation| NotNan::new(elevation).unwrap())
                    })
                    .collect();
                let extended_vertex_elevations = vertex_elevations.split_off(6);

                // Limit to a single river edge coming out of each tile, in the direction of the
                // vertex with the lowest elevation.
                let Some((vertex_min, _elevation_min)) =
                    zip(vertex_elevations, extended_vertex_elevations.iter())
                        .enumerate()
                        .filter_map(|(i, (elevation, dest_elevation))| {
                            if let (Some(elevation), &Some(dest_elevation)) =
                                (elevation, dest_elevation)
                            {
                                // Avoid creating river edges going to the same or higher elevation.
                                if dest_elevation > elevation {
                                    Some((i, elevation))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .reduce(|(vertex_min, elevation_min), (i, elevation)| {
                            let elevation_min = elevation_min.min(elevation);
                            let vertex_min = if elevation_min == elevation {
                                i
                            } else {
                                vertex_min
                            };
                            (vertex_min, elevation_min)
                        })
                else {
                    continue;
                };

                let edge_a = vertex_min;
                let edge_b = (edge_a + 1) % 6;
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

                let (tile_pos_a, tile_pos_b) = (
                    neighbor_positions.get(HEX_DIRECTIONS[edge_a]),
                    neighbor_positions.get(HEX_DIRECTIONS[edge_b]),
                );
                if tile_pos_a.is_none() && tile_pos_b.is_none() {
                    // Avoid creating river edges sticking out from the map.
                    continue;
                }

                let source =
                    AxialPos::from_tile_pos_given_coord_system(&tile_pos, HexCoordSystem::RowOdd);
                let destination = source + HexVertexDirection::OFFSETS[edge_a];

                if river_edges.iter().any(|river_edge| {
                    river_edge.source == source && river_edge.destination == destination
                }) {
                    // Skip if river edge already exists.
                    continue;
                }

                let new_river_edge = RiverEdge {
                    source,
                    destination,
                    stream_order: StreamOrder(1),
                };
                river_edges.push(new_river_edge);

                // Recursively merge streams if the new river edge is at a
                // confluence with an existing river edge.
                let mut new_river_edge = Some(new_river_edge);
                while let Some(RiverEdge {
                    source,
                    destination,
                    stream_order,
                    ..
                }) = new_river_edge.take()
                {
                    let tile_pos = source.as_tile_pos_given_coord_system(HexCoordSystem::RowOdd);
                    let vertex_direction: HexVertexDirection = (source, destination)
                        .try_into()
                        .expect("`(source, destination)` should match a valid offset");
                    let neighbor_positions =
                        HexNeighbors::get_neighboring_positions_row_odd(&tile_pos, map_size);
                    let edge_a = vertex_direction as usize;
                    let edge_b = (edge_a + 1) % 6;

                    if let Some(&tile_pos_a) = neighbor_positions.get(HEX_DIRECTIONS[edge_a]) {
                        let axial_pos_a = AxialPos::from_tile_pos_given_coord_system(
                            &tile_pos_a,
                            HexCoordSystem::RowOdd,
                        );
                        let neighbor_positions =
                            HexNeighbors::get_neighboring_positions_row_odd(&tile_pos_a, map_size);
                        if let Some(&tile_pos_aa) = neighbor_positions.get(HEX_DIRECTIONS[edge_a]) {
                            let axial_pos_aa = AxialPos::from_tile_pos_given_coord_system(
                                &tile_pos_aa,
                                HexCoordSystem::RowOdd,
                            );
                            let confluence_destination =
                                source + HexVertexDirection::OFFSETS[(edge_a + 2) % 6];
                            debug!(
                                ?tile_pos,
                                ?vertex_direction,
                                ?tile_pos_a,
                                ?axial_pos_a,
                                ?tile_pos_aa,
                                ?axial_pos_aa,
                                ?confluence_destination,
                                "checking for river confluence"
                            );
                            if let Some(confluence_river_edge) =
                                river_edges.iter().find(|river_edge| {
                                    river_edge.source == axial_pos_aa
                                        && river_edge.destination == confluence_destination
                                })
                            {
                                // debug!(?confluence_river_edge, "found river confluence");
                                let merged_destination =
                                    source + HexVertexDirection::OFFSETS[(edge_a + 1) % 6];
                                if river_edges.iter().any(|river_edge| {
                                    river_edge.source == axial_pos_a
                                        && river_edge.destination == merged_destination
                                }) {
                                    // Skip if river edge already exists.
                                    continue;
                                }

                                let merged_stream_order =
                                    stream_order + confluence_river_edge.stream_order;
                                let river_edge = RiverEdge {
                                    source: axial_pos_a,
                                    destination: merged_destination,
                                    stream_order: merged_stream_order,
                                };
                                river_edges.push(river_edge);
                                new_river_edge = Some(river_edge);
                            }
                        }
                    }

                    if let Some(&tile_pos_b) = neighbor_positions.get(HEX_DIRECTIONS[edge_b]) {
                        let axial_pos_b = AxialPos::from_tile_pos_given_coord_system(
                            &tile_pos_b,
                            HexCoordSystem::RowOdd,
                        );
                        let neighbor_positions =
                            HexNeighbors::get_neighboring_positions_row_odd(&tile_pos_b, map_size);
                        if let Some(&tile_pos_bb) = neighbor_positions.get(HEX_DIRECTIONS[edge_b]) {
                            let axial_pos_bb = AxialPos::from_tile_pos_given_coord_system(
                                &tile_pos_bb,
                                HexCoordSystem::RowOdd,
                            );
                            let confluence_destination =
                                source + HexVertexDirection::OFFSETS[(edge_b + 3) % 6];
                            // debug!(
                            //     ?tile_pos,
                            //     ?vertex_direction,
                            //     ?tile_pos_b,
                            //     ?axial_pos_b,
                            //     ?tile_pos_bb,
                            //     ?axial_pos_bb,
                            //     ?confluence_destination,
                            //     "checking for river confluence"
                            // );
                            if let Some(confluence_river_edge) =
                                river_edges.iter().find(|river_edge| {
                                    river_edge.source == axial_pos_bb
                                        && river_edge.destination == confluence_destination
                                })
                            {
                                // debug!(?confluence_river_edge, "found river confluence");
                                let merged_destination =
                                    source + HexVertexDirection::OFFSETS[(edge_b + 4) % 6];
                                if river_edges.iter().any(|river_edge| {
                                    river_edge.source == axial_pos_b
                                        && river_edge.destination == merged_destination
                                }) {
                                    // Skip if river edge already exists.
                                    continue;
                                }

                                let merged_stream_order =
                                    stream_order + confluence_river_edge.stream_order;
                                let river_edge = RiverEdge {
                                    source: axial_pos_b,
                                    destination: merged_destination,
                                    stream_order: merged_stream_order,
                                };
                                river_edges.push(river_edge);
                                new_river_edge = Some(river_edge);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut grouped_river_edges: Vec<Vec<RiverEdge>> = vec![];

    let mut river_edges: Vec<Option<RiverEdge>> = river_edges.into_iter().map(Some).collect();

    // Recursively group adjacent river edges.
    for river_edge in river_edges.iter_mut() {
        // TODO: Actually perform grouping.
        grouped_river_edges.push(vec![river_edge.take().unwrap()]);
    }

    // Remove rivers which are too short.
    // grouped_river_edges.retain(|river_edges| river_edges.len() >=
    // River::MIN_LEN);

    // debug!(?grouped_river_edges, "generated rivers");

    let mut river_hex_edges_map: HashMap<TilePos, RiverHexEdges> = HashMap::new();

    for river_edges in grouped_river_edges {
        for RiverEdge {
            source,
            destination,
            ..
        } in river_edges
        {
            let tile_pos = source.as_tile_pos_given_coord_system(HexCoordSystem::RowOdd);
            let vertex_direction: HexVertexDirection = (source, destination)
                .try_into()
                .expect("`(source, destination)` should match a valid offset");
            let neighbor_positions =
                HexNeighbors::get_neighboring_positions_row_odd(&tile_pos, map_size);

            let edge_a = vertex_direction as usize;
            let edge_b = (edge_a + 1) % 6;

            if let Some(tile_pos) = neighbor_positions.get(HEX_DIRECTIONS[edge_a]) {
                let river_hex_edges = river_hex_edges_map
                    .entry(*tile_pos)
                    .or_insert(BitArray::<_>::ZERO);
                let river_edge = (edge_a + 2) % 6;
                river_hex_edges.set(river_edge, true);
            }

            if let Some(tile_pos) = neighbor_positions.get(HEX_DIRECTIONS[edge_b]) {
                let river_hex_edges = river_hex_edges_map
                    .entry(*tile_pos)
                    .or_insert(BitArray::<_>::ZERO);
                let river_edge = (edge_b + 4) % 6;
                river_hex_edges.set(river_edge, true);
            }
        }
    }

    for (tile_pos, river_hex_edges) in river_hex_edges_map {
        let tile_entity = commands
            .spawn(TileBundle {
                position: tile_pos,
                tilemap_id: TilemapId(river_tilemap_entity),
                texture_index: TileTextureIndex(river_hex_edges.load()),
                ..Default::default()
            })
            .insert(RiverLayer)
            .id();
        river_tile_storage.set(&tile_pos, tile_entity);
    }
}

pub fn upgrade_camera(mut commands: Commands, camera_query: Single<(Entity,), With<Camera2d>>) {
    let (camera_entity,) = camera_query.into_inner();

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
