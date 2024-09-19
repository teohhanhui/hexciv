use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::color::palettes;
use bevy::ecs::system::{RunSystemOnce as _, SystemState};
use bevy::prelude::*;
use bevy_ecs_tilemap::helpers::hex_grid::neighbors::{HexNeighbors, HEX_DIRECTIONS};
use bevy_ecs_tilemap::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use bitvec::prelude::*;
use fastlem_random_terrain::{generate_terrain, Site2D, Terrain2D};
use fastrand_contrib::RngExt as _;
#[cfg(debug_assertions)]
use hexciv::actions::DebugAction;
use hexciv::actions::{GlobalAction, UnitAction};
use hexciv::states::TurnState;
use hexciv::types::Civilization;
use indexmap::IndexSet;
use itertools::{chain, repeat_n, Itertools as _};
use leafwing_input_manager::common_conditions::{action_just_pressed, action_toggle_active};
use leafwing_input_manager::prelude::*;
use ordered_float::NotNan;
use strum::VariantArray as _;

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

#[cfg(debug_assertions)]
const TILE_LABEL_Z_INDEX: f32 = 3.0;

const FRIGID_ZONE_TILE_CHOICES: [BaseTerrain; 2] = [BaseTerrain::Tundra, BaseTerrain::Snow];
const TEMPERATE_ZONE_TILE_CHOICES: [BaseTerrain; 4] = [
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Grassland,
];
const SUBTROPICS_TILE_CHOICES: [BaseTerrain; 7] = [
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Grassland,
    BaseTerrain::Grassland,
    BaseTerrain::Desert,
    BaseTerrain::Desert,
];
const TROPICS_TILE_CHOICES: [BaseTerrain; 7] = [
    BaseTerrain::Plains,
    BaseTerrain::Plains,
    BaseTerrain::Grassland,
    BaseTerrain::Grassland,
    BaseTerrain::Grassland,
    BaseTerrain::Grassland,
    BaseTerrain::Desert,
];

const OASIS_CHOICES: [bool; 5] = [true, false, false, false, false];
const ICE_CHOICES: [bool; 4] = [true, true, true, false];

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
enum BaseTerrain {
    Plains = 0,
    Grassland = 1,
    Desert = 2,
    Tundra = 3,
    Snow = 4,
    // PlainsHills = 5,
    // GrasslandHills = 6,
    // DesertHills = 7,
    // TundraHills = 8,
    // SnowHills = 9,
    // PlainsMountains = 10,
    // GrasslandMountains = 11,
    // DesertMountains = 12,
    // TundraMountains = 13,
    // SnowMountains = 14,
    Coast = 15,
    Ocean = 16,
}

#[derive(Copy, Clone)]
#[repr(u32)]
enum BaseTerrainVariant {
    Hills = 5,
    Mountains = 10,
}

type RiverEdges = BitArr!(for 6, in u16);

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
enum TerrainFeatures {
    // Woods = 0,
    // Rainforest = 1,
    // Marsh = 2,
    // Floodplains = 3,
    Oasis = 4,
    // Cliffs = 5,
    Ice = 6,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
enum UnitSelection {
    Active = 0,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
enum UnitState {
    Ready = 0,
    // OutOfMoves = 1,
    OutOfOrders = 2,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
enum LandMilitaryUnit {
    Settler = 0,
    Warrior = 1,
}

enum EarthLatitude {
    ArticCirle,
    TropicOfCancer,
    TropicOfCapricorn,
    AntarcticCircle,
}

#[derive(Deref, Resource)]
struct FontHandle(Handle<Font>);

#[derive(Resource)]
struct MapRng(fastrand::Rng);

#[derive(Resource)]
struct GameRng(fastrand::Rng);

#[derive(Resource)]
struct MapTerrain(Terrain2D);

#[derive(Resource)]
struct CursorPos(Vec2);

#[derive(Component)]
struct BaseTerrainLayer;

#[derive(Component)]
struct RiverLayer;

#[derive(Component)]
struct TerrainFeaturesLayer;

#[derive(Component)]
struct UnitSelectionLayer;

#[derive(Component)]
struct UnitStateLayer;

#[derive(Component)]
struct LandMilitaryUnitLayer;

#[derive(Component)]
struct TileLabel(Entity);

#[derive(Component)]
struct HighlightedLabel;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
struct SpawnTilemapSet;

impl EarthLatitude {
    pub const fn latitude(&self) -> f64 {
        match self {
            EarthLatitude::ArticCirle => 66.57,
            EarthLatitude::TropicOfCancer => 23.43,
            EarthLatitude::TropicOfCapricorn => -23.43,
            EarthLatitude::AntarcticCircle => -66.57,
        }
    }
}

impl FromWorld for FontHandle {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self(asset_server.load("fonts/NotoSans/NotoSans-Regular.ttf"))
    }
}

impl FromWorld for MapRng {
    fn from_world(_world: &mut World) -> Self {
        let rng = fastrand::Rng::new();
        Self(rng)
    }
}

impl FromWorld for GameRng {
    fn from_world(_world: &mut World) -> Self {
        let rng = fastrand::Rng::new();
        Self(rng)
    }
}

impl Default for CursorPos {
    fn default() -> Self {
        // Initialize the cursor pos at some far away place. It will get updated
        // correctly when the cursor moves.
        Self(Vec2::new(-1000.0, -1000.0))
    }
}

impl RiverLayer {
    const Z_INDEX: f32 = 1.0;
}

impl TerrainFeaturesLayer {
    const Z_INDEX: f32 = 2.0;
}

impl UnitSelectionLayer {
    const Z_INDEX: f32 = 4.0;
}

impl UnitStateLayer {
    const Z_INDEX: f32 = 5.0;
}

impl LandMilitaryUnitLayer {
    const Z_INDEX: f32 = 6.0;
}

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Hexciv".to_owned(),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .set(ImagePlugin::default_nearest()),
    )
    .add_plugins((
        InputManagerPlugin::<GlobalAction>::default(),
        InputManagerPlugin::<UnitAction>::default(),
    ))
    .add_plugins(PanCamPlugin)
    .add_plugins(TilemapPlugin)
    .init_resource::<FontHandle>()
    .init_resource::<MapRng>()
    .init_resource::<GameRng>()
    .init_resource::<ActionState<GlobalAction>>()
    .init_resource::<ActionState<UnitAction>>()
    .init_resource::<CursorPos>()
    .insert_resource(GlobalAction::input_map())
    .insert_resource(UnitAction::input_map())
    .init_state::<TurnState>()
    .add_systems(
        Startup,
        (spawn_tilemap, post_spawn_tilemap)
            .chain()
            .in_set(SpawnTilemapSet),
    )
    .add_systems(Startup, spawn_starting_units.after(SpawnTilemapSet))
    .add_systems(
        OnEnter(TurnState::Playing),
        (cycle_ready_unit, focus_camera_on_active_unit).chain(),
    )
    .add_systems(
        Update,
        (cycle_ready_unit, focus_camera_on_active_unit)
            .chain()
            .run_if(action_just_pressed(GlobalAction::PreviousReadyUnit)),
    )
    .add_systems(
        Update,
        (cycle_ready_unit, focus_camera_on_active_unit)
            .chain()
            .run_if(action_just_pressed(GlobalAction::NextReadyUnit)),
    )
    .add_systems(
        Update,
        mark_active_unit_out_of_orders.run_if(action_just_pressed(UnitAction::SkipTurn)),
    )
    .add_systems(Update, update_cursor_pos);

    #[cfg(debug_assertions)]
    {
        app.add_plugins(InputManagerPlugin::<DebugAction>::default())
            .init_resource::<ActionState<DebugAction>>()
            .insert_resource(DebugAction::input_map())
            .add_systems(
                Update,
                (
                    (
                        show_tile_labels
                            .run_if(action_toggle_active(false, DebugAction::ShowTileLabels)),
                        hide_tile_labels
                            .run_if(action_toggle_active(true, DebugAction::ShowTileLabels)),
                    ),
                    highlight_tile_labels.after(update_cursor_pos),
                )
                    .chain(),
            );
    }

    app.run();
}

/// Generates the initial tilemap.
fn spawn_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut map_rng: ResMut<MapRng>,
) {
    commands.spawn(Camera2dBundle::default()).insert(PanCam {
        grab_buttons: vec![MouseButton::Left],
        min_scale: 1.0,
        max_scale: 10.0,
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

    let image_handles = vec![
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
    let texture_vec = TilemapTexture::Vector(image_handles);

    let map_size = TilemapSize {
        x: MAP_SIDE_LENGTH_X,
        y: MAP_SIDE_LENGTH_Y,
    };

    let rng = &mut map_rng.0;
    info!(seed = rng.get_seed(), "map seed");

    let terrain = {
        let config = fastlem_random_terrain::Config {
            seed: rng.u32(..),
            land_ratio: rng.f64_range(0.29..=0.6),
            ..Default::default()
        };
        info!(?config, "fastlem-random-terrain config");
        generate_terrain(&config, BOUND_MIN, BOUND_MAX, BOUND_RANGE)
    };

    let mut tile_storage = TileStorage::empty(map_size);
    let tilemap_entity = commands.spawn_empty().id();
    let tilemap_id = TilemapId(tilemap_entity);

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
            let elevation = elevations.iter().sum::<NotNan<_>>() / elevations.len() as f64;
            let texture_index = if elevation < NotNan::new(0.05).unwrap() {
                TileTextureIndex(BaseTerrain::Ocean as u32)
            } else {
                let latitude = NotNan::new(-90.0).unwrap()
                    + NotNan::new(180.0).unwrap()
                        * ((NotNan::from(tile_pos.y) + 0.5) / NotNan::from(map_size.y));

                let base_terrain = choose_base_terrain_by_latitude(rng, latitude);

                TileTextureIndex(if elevation >= NotNan::new(25.0).unwrap() {
                    base_terrain as u32 + BaseTerrainVariant::Mountains as u32
                } else if elevation >= NotNan::new(5.0).unwrap() {
                    base_terrain as u32 + BaseTerrainVariant::Hills as u32
                } else {
                    base_terrain as u32
                })
            };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    tilemap_id,
                    texture_index,
                    ..Default::default()
                })
                .insert(BaseTerrainLayer)
                .id();
            tile_storage.set(&tile_pos, tile_entity);
        }
    }

    commands
        .entity(tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: tile_storage,
            texture: texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(&map_size, &GRID_SIZE, &MAP_TYPE, 0.0),
            ..Default::default()
        })
        .insert(BaseTerrainLayer);

    let image_handles = {
        let image_map: BTreeMap<u16, Handle<Image>> = repeat_n([true, false].into_iter(), 6)
            .multi_cartesian_product()
            .map(|data| {
                let mut bits: RiverEdges = BitArray::<_>::ZERO;
                for (i, v) in data.iter().enumerate() {
                    bits.set(i, *v);
                }
                (
                    bits.load(),
                    asset_server.load(format!(
                        "tiles/river-{edges}.png",
                        edges = data
                            .iter()
                            .enumerate()
                            .map(|(i, v)| if *v { i.to_string() } else { "x".to_owned() })
                            .join("")
                    )),
                )
            })
            .collect();
        let size = usize::from(*image_map.last_key_value().unwrap().0) + 1;
        let mut image_vec = vec![asset_server.load("tiles/transparent.png"); size];
        for (key, image) in image_map {
            image_vec[usize::from(key)] = image;
        }
        image_vec
    };
    let texture_vec = TilemapTexture::Vector(image_handles);

    let tile_storage = TileStorage::empty(map_size);
    let tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: tile_storage,
            texture: texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(&map_size, &GRID_SIZE, &MAP_TYPE, 0.0)
                * Transform::from_xyz(0.0, 0.0, RiverLayer::Z_INDEX),
            ..Default::default()
        })
        .insert(RiverLayer);

    let image_handles = vec![
        // TODO: woods
        asset_server.load("tiles/transparent.png"),
        // TODO: rainforest
        asset_server.load("tiles/transparent.png"),
        // TODO: marsh
        asset_server.load("tiles/transparent.png"),
        // TODO: floodplains
        asset_server.load("tiles/transparent.png"),
        asset_server.load("tiles/oasis.png"),
        // TODO: cliffs
        asset_server.load("tiles/transparent.png"),
        asset_server.load("tiles/ice.png"),
    ];
    let texture_vec = TilemapTexture::Vector(image_handles);

    let tile_storage = TileStorage::empty(map_size);
    let tilemap_entity = commands.spawn_empty().id();

    commands
        .entity(tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: map_size,
            storage: tile_storage,
            texture: texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(&map_size, &GRID_SIZE, &MAP_TYPE, 0.0)
                * Transform::from_xyz(0.0, 0.0, TerrainFeaturesLayer::Z_INDEX),
            ..Default::default()
        })
        .insert(TerrainFeaturesLayer);

    commands.insert_resource(MapTerrain(terrain));
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn post_spawn_tilemap(
    mut commands: Commands,
    mut map_rng: ResMut<MapRng>,
    map_terrain: Res<MapTerrain>,
    base_terrain_tilemap_query: Query<
        (&TilemapSize, &TileStorage),
        (
            With<BaseTerrainLayer>,
            Without<RiverLayer>,
            Without<TerrainFeaturesLayer>,
        ),
    >,
    mut river_tilemap_query: Query<
        (Entity, &mut TileStorage),
        (
            With<RiverLayer>,
            Without<BaseTerrainLayer>,
            Without<TerrainFeaturesLayer>,
        ),
    >,
    mut terrain_features_tilemap_query: Query<
        (Entity, &mut TileStorage),
        (
            With<TerrainFeaturesLayer>,
            Without<BaseTerrainLayer>,
            Without<RiverLayer>,
        ),
    >,
    mut base_terrain_tile_query: Query<
        &mut TileTextureIndex,
        (With<BaseTerrainLayer>, Without<TerrainFeaturesLayer>),
    >,
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
            let tile_texture = *base_terrain_tile_query.get(tile_entity).unwrap();
            let neighbor_positions =
                HexNeighbors::get_neighboring_positions_row_odd(&tile_pos, map_size);
            let neighbor_entities = neighbor_positions.entities(base_terrain_tile_storage);

            if tile_texture.0 == BaseTerrain::Ocean as u32
                && neighbor_entities.iter().any(|neighbor_entity| {
                    let tile_texture = base_terrain_tile_query.get(*neighbor_entity).unwrap();
                    ![BaseTerrain::Ocean as u32, BaseTerrain::Coast as u32]
                        .contains(&tile_texture.0)
                })
            {
                let mut tile_texture = base_terrain_tile_query.get_mut(tile_entity).unwrap();
                tile_texture.0 = BaseTerrain::Coast as u32;
            }

            if tile_texture.0 == BaseTerrain::Desert as u32 && rng.choice(OASIS_CHOICES).unwrap() {
                let tile_entity = commands
                    .spawn(TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(terrain_features_tilemap_entity),
                        texture_index: TileTextureIndex(TerrainFeatures::Oasis as u32),
                        ..Default::default()
                    })
                    .insert(TerrainFeaturesLayer)
                    .id();
                terrain_features_tile_storage.set(&tile_pos, tile_entity);
            }

            if [BaseTerrain::Ocean as u32, BaseTerrain::Coast as u32].contains(&tile_texture.0) {
                let latitude = NotNan::new(-90.0).unwrap()
                    + NotNan::new(180.0).unwrap()
                        * ((NotNan::from(tile_pos.y) + 0.5) / NotNan::from(map_size.y));

                if (*latitude >= EarthLatitude::ArticCirle.latitude()
                    || *latitude <= EarthLatitude::AntarcticCircle.latitude())
                    && rng.choice(ICE_CHOICES).unwrap()
                {
                    let tile_entity = commands
                        .spawn(TileBundle {
                            position: tile_pos,
                            tilemap_id: TilemapId(terrain_features_tilemap_entity),
                            texture_index: TileTextureIndex(TerrainFeatures::Ice as u32),
                            ..Default::default()
                        })
                        .insert(TerrainFeaturesLayer)
                        .id();
                    terrain_features_tile_storage.set(&tile_pos, tile_entity);
                }
            }

            if ![
                BaseTerrain::Ocean as u32,
                BaseTerrain::Coast as u32,
                // Exclude lowlands and deserts as river source.
                BaseTerrain::Plains as u32,
                BaseTerrain::Grassland as u32,
                BaseTerrain::Desert as u32,
                BaseTerrain::Desert as u32 + BaseTerrainVariant::Hills as u32,
                BaseTerrain::Desert as u32 + BaseTerrainVariant::Mountains as u32,
                BaseTerrain::Tundra as u32,
                BaseTerrain::Snow as u32,
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
                    let tile_texture = *base_terrain_tile_query
                        .get(*edge_adjacent_tile_entity)
                        .unwrap();
                    if [BaseTerrain::Ocean as u32, BaseTerrain::Coast as u32]
                        .contains(&tile_texture.0)
                    {
                        // Avoid creating river edges parallel to the sea shore / lake shore.
                        continue;
                    }
                }
                if let Some(edge_adjacent_tile_entity) =
                    neighbor_entities.get(HEX_DIRECTIONS[edge_b])
                {
                    let tile_texture = *base_terrain_tile_query
                        .get(*edge_adjacent_tile_entity)
                        .unwrap();
                    if [BaseTerrain::Ocean as u32, BaseTerrain::Coast as u32]
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
                texture_index: TileTextureIndex(u32::from(river_edges.load::<u16>())),
                ..Default::default()
            })
            .insert(RiverLayer)
            .id();
        river_tile_storage.set(&tile_pos, tile_entity);
    }
}

#[allow(clippy::type_complexity)]
fn spawn_starting_units(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_turn_state: ResMut<NextState<TurnState>>,
    mut game_rng: ResMut<GameRng>,
    base_terrain_tilemap_query: Query<
        (&TilemapSize, &TileStorage),
        (
            With<BaseTerrainLayer>,
            Without<RiverLayer>,
            Without<TerrainFeaturesLayer>,
        ),
    >,
    base_terrain_tile_query: Query<
        &TileTextureIndex,
        (
            With<BaseTerrainLayer>,
            Without<RiverLayer>,
            Without<TerrainFeaturesLayer>,
        ),
    >,
) {
    let image_handles = vec![asset_server.load("units/active.png")];
    let unit_selection_texture_vec = TilemapTexture::Vector(image_handles);

    let image_handles = vec![
        asset_server.load("units/ready.png"),
        asset_server.load("units/out.png"),
        asset_server.load("units/out.png"),
    ];
    let unit_state_texture_vec = TilemapTexture::Vector(image_handles);

    let image_handles = vec![
        asset_server.load("units/settler.png"),
        asset_server.load("units/warrior.png"),
    ];
    let land_military_unit_texture_vec = TilemapTexture::Vector(image_handles);

    let rng = &mut game_rng.0;
    info!(seed = rng.get_seed(), "game seed");

    let (map_size, base_terrain_tile_storage) = base_terrain_tilemap_query.get_single().unwrap();

    let civ = rng.choice(Civilization::VARIANTS).unwrap();

    let mut allowable_starting_positions = HashSet::new();

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let tile_entity = base_terrain_tile_storage.get(&tile_pos).unwrap();
            let tile_texture = *base_terrain_tile_query.get(tile_entity).unwrap();

            if [
                BaseTerrain::Ocean as u32,
                BaseTerrain::Coast as u32,
                // Exclude mountains.
                BaseTerrain::Plains as u32 + BaseTerrainVariant::Mountains as u32,
                BaseTerrain::Grassland as u32 + BaseTerrainVariant::Mountains as u32,
                BaseTerrain::Desert as u32 + BaseTerrainVariant::Mountains as u32,
                BaseTerrain::Tundra as u32 + BaseTerrainVariant::Mountains as u32,
                BaseTerrain::Snow as u32 + BaseTerrainVariant::Mountains as u32,
            ]
            .contains(&tile_texture.0)
            {
                continue;
            }

            allowable_starting_positions.insert(tile_pos);
        }
    }

    let mut settler_tile_pos;
    let warrior_tile_pos;
    loop {
        settler_tile_pos = *rng
            .choice(&allowable_starting_positions)
            .expect("the map should have enough land tiles");
        let neighbor_positions: HashSet<_> =
            HexNeighbors::get_neighboring_positions_row_odd(&settler_tile_pos, map_size)
                .iter()
                .copied()
                .collect();
        let allowable_neighbors: HashSet<_> = neighbor_positions
            .intersection(&allowable_starting_positions)
            .copied()
            .collect();
        if allowable_neighbors.is_empty() {
            continue;
        }
        warrior_tile_pos = *rng.choice(&allowable_neighbors).unwrap();
        break;
    }

    let mut unit_state_tile_storage = TileStorage::empty(*map_size);
    let unit_state_tilemap_entity = commands.spawn_empty().id();
    let mut land_military_unit_tile_storage = TileStorage::empty(*map_size);
    let land_military_unit_tilemap_entity = commands.spawn_empty().id();

    // Spawn settler.
    {
        let tile_entity = commands
            .spawn(TileBundle {
                position: settler_tile_pos,
                tilemap_id: TilemapId(unit_state_tilemap_entity),
                texture_index: TileTextureIndex(UnitState::Ready as u32),
                color: TileColor(civ.colors()[0].into()),
                ..Default::default()
            })
            .insert(UnitStateLayer)
            .id();
        unit_state_tile_storage.set(&settler_tile_pos, tile_entity);
        let tile_entity = commands
            .spawn(TileBundle {
                position: settler_tile_pos,
                tilemap_id: TilemapId(land_military_unit_tilemap_entity),
                texture_index: TileTextureIndex(LandMilitaryUnit::Settler as u32),
                color: TileColor(civ.colors()[1].into()),
                ..Default::default()
            })
            .insert(LandMilitaryUnitLayer)
            .id();
        land_military_unit_tile_storage.set(&settler_tile_pos, tile_entity);
    }

    // Spawn warrior.
    {
        let tile_entity = commands
            .spawn(TileBundle {
                position: warrior_tile_pos,
                tilemap_id: TilemapId(unit_state_tilemap_entity),
                texture_index: TileTextureIndex(UnitState::Ready as u32),
                color: TileColor(civ.colors()[0].into()),
                ..Default::default()
            })
            .insert(UnitStateLayer)
            .id();
        unit_state_tile_storage.set(&warrior_tile_pos, tile_entity);
        let tile_entity = commands
            .spawn(TileBundle {
                position: warrior_tile_pos,
                tilemap_id: TilemapId(land_military_unit_tilemap_entity),
                texture_index: TileTextureIndex(LandMilitaryUnit::Warrior as u32),
                color: TileColor(civ.colors()[1].into()),
                ..Default::default()
            })
            .insert(LandMilitaryUnitLayer)
            .id();
        land_military_unit_tile_storage.set(&settler_tile_pos, tile_entity);
    }

    {
        let tile_storage = TileStorage::empty(*map_size);
        let tilemap_entity = commands.spawn_empty().id();

        commands
            .entity(tilemap_entity)
            .insert(TilemapBundle {
                grid_size: GRID_SIZE,
                size: *map_size,
                storage: tile_storage,
                texture: unit_selection_texture_vec,
                tile_size: TILE_SIZE,
                map_type: MAP_TYPE,
                transform: get_tilemap_center_transform(map_size, &GRID_SIZE, &MAP_TYPE, 0.0)
                    * Transform::from_xyz(0.0, 0.0, UnitSelectionLayer::Z_INDEX),
                ..Default::default()
            })
            .insert(UnitSelectionLayer);
    }

    commands
        .entity(unit_state_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: *map_size,
            storage: unit_state_tile_storage,
            texture: unit_state_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(map_size, &GRID_SIZE, &MAP_TYPE, 0.0)
                * Transform::from_xyz(0.0, 0.0, UnitStateLayer::Z_INDEX),
            ..Default::default()
        })
        .insert(UnitStateLayer);

    commands
        .entity(land_military_unit_tilemap_entity)
        .insert(TilemapBundle {
            grid_size: GRID_SIZE,
            size: *map_size,
            storage: land_military_unit_tile_storage,
            texture: land_military_unit_texture_vec,
            tile_size: TILE_SIZE,
            map_type: MAP_TYPE,
            transform: get_tilemap_center_transform(map_size, &GRID_SIZE, &MAP_TYPE, 0.0)
                * Transform::from_xyz(0.0, 0.0, LandMilitaryUnitLayer::Z_INDEX),
            ..Default::default()
        })
        .insert(LandMilitaryUnitLayer);

    next_turn_state.set(TurnState::Playing);
}

#[allow(clippy::type_complexity)]
fn cycle_ready_unit(
    mut commands: Commands,
    global_action_state: Res<ActionState<GlobalAction>>,
    mut unit_selection_tilemap_query: Query<(Entity, &mut TileStorage), With<UnitSelectionLayer>>,
    mut unit_selection_tile_query: Query<
        (&mut TilePos, &TileTextureIndex),
        (With<UnitSelectionLayer>, Without<UnitStateLayer>),
    >,
    unit_state_tile_query: Query<
        (&TilePos, &TileTextureIndex),
        (With<UnitStateLayer>, Without<UnitSelectionLayer>),
    >,
) {
    // TODO: Restrict to the units controlled by the current player.
    let ready_unit_tile_positions: IndexSet<_> = unit_state_tile_query
        .iter()
        .filter_map(|(&tile_pos, &tile_texture)| {
            if tile_texture.0 == UnitState::Ready as u32 {
                Some(tile_pos)
            } else {
                None
            }
        })
        .collect();
    if ready_unit_tile_positions.is_empty() {
        // There are no ready units to cycle to.
        return;
    }
    let active_unit_tile_pos =
        unit_selection_tile_query
            .iter_mut()
            .find_map(|(tile_pos, &tile_texture)| {
                if tile_texture.0 == UnitSelection::Active as u32 {
                    Some(tile_pos)
                } else {
                    None
                }
            });

    if let Some(mut active_unit_tile_pos) = active_unit_tile_pos {
        // Move the unit selection tile to the previous / next ready unit.

        // TODO: Restrict to the units controlled by the current player.
        let unit_tile_positions: Vec<_> = unit_state_tile_query
            .iter()
            .map(|(&tile_pos, _)| tile_pos)
            .collect();

        if global_action_state.just_pressed(&GlobalAction::PreviousReadyUnit) {
            let previous_unit_tile_positions: IndexSet<_> = unit_tile_positions
                .into_iter()
                .rev()
                .skip_while(|&tile_pos| tile_pos != *active_unit_tile_pos)
                .skip(1)
                .collect();
            if let Some(tile_pos) = previous_unit_tile_positions
                .intersection(&ready_unit_tile_positions)
                .next()
            {
                *active_unit_tile_pos = *tile_pos;
            }
        } else if global_action_state.just_pressed(&GlobalAction::NextReadyUnit) {
            let next_unit_tile_positions: IndexSet<_> = unit_tile_positions
                .into_iter()
                .skip_while(|&tile_pos| tile_pos != *active_unit_tile_pos)
                .skip(1)
                .collect();
            if let Some(tile_pos) = next_unit_tile_positions
                .intersection(&ready_unit_tile_positions)
                .next()
            {
                *active_unit_tile_pos = *tile_pos;
            }
        } else {
            // Not cycling units.
            return;
        }
    } else {
        // Spawn a new unit selection tile, since there was no currently active unit.

        let (tilemap_entity, mut tile_storage) =
            unit_selection_tilemap_query.get_single_mut().unwrap();

        let tile_pos = ready_unit_tile_positions[0];
        let tile_entity = commands
            .spawn(TileBundle {
                position: tile_pos,
                tilemap_id: TilemapId(tilemap_entity),
                texture_index: TileTextureIndex(UnitSelection::Active as u32),
                ..Default::default()
            })
            .insert(UnitSelectionLayer)
            .id();
        tile_storage.set(&tile_pos, tile_entity);
    }
}

#[allow(clippy::type_complexity)]
fn mark_active_unit_out_of_orders(
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex),
        (With<UnitSelectionLayer>, Without<UnitStateLayer>),
    >,
    mut unit_state_tile_query: Query<
        (&TilePos, &mut TileTextureIndex),
        (With<UnitStateLayer>, Without<UnitSelectionLayer>),
    >,
) {
    let Some(active_unit_tile_pos) =
        unit_selection_tile_query
            .iter()
            .find_map(|(tile_pos, tile_texture)| {
                if tile_texture.0 == UnitSelection::Active as u32 {
                    Some(*tile_pos)
                } else {
                    None
                }
            })
    else {
        // No active unit selection.
        return;
    };

    for (tile_pos, mut tile_texture) in unit_state_tile_query.iter_mut() {
        if *tile_pos == active_unit_tile_pos && tile_texture.0 == UnitState::Ready as u32 {
            tile_texture.0 = UnitState::OutOfOrders as u32;
        }
    }
}

#[allow(clippy::type_complexity)]
fn focus_camera_on_active_unit(
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<UnitSelectionLayer>)>,
    unit_selection_tilemap_query: Query<
        (&Transform, &TilemapType, &TilemapGridSize),
        (With<UnitSelectionLayer>, Without<Camera2d>),
    >,
    unit_selection_tile_query: Query<(&TilePos, &TileTextureIndex), With<UnitSelectionLayer>>,
) {
    let mut camera_transform = camera_query.get_single_mut().unwrap();
    let (map_transform, map_type, grid_size) = unit_selection_tilemap_query.get_single().unwrap();
    for (tile_pos, tile_texture) in unit_selection_tile_query.iter() {
        if tile_texture.0 == UnitSelection::Active as u32 {
            let tile_center = tile_pos
                .center_in_world(grid_size, map_type)
                .extend(UnitSelectionLayer::Z_INDEX);
            let tile_translation = map_transform.translation + tile_center;
            camera_transform.translation = tile_translation.with_z(camera_transform.translation.z);
            break;
        }
    }
}

#[cfg(debug_assertions)]
#[allow(clippy::type_complexity)]
fn show_tile_labels(
    world: &mut World,
    tile_label_query: &mut QueryState<(), With<TileLabel>>,
    system_state: &mut SystemState<(Query<&TileLabel>, Query<&mut Visibility, With<Text>>)>,
) {
    if tile_label_query.iter(world).next().is_none() {
        world.run_system_once(spawn_tile_labels);
    }

    {
        let (tile_label_query, mut text_query) = system_state.get_mut(world);

        for tile_label in tile_label_query.iter() {
            if let Ok(mut visibility) = text_query.get_mut(tile_label.0) {
                *visibility = Visibility::Visible;
            }
        }
    }
}

#[cfg(debug_assertions)]
fn hide_tile_labels(
    tile_label_query: Query<&TileLabel>,
    mut text_query: Query<&mut Visibility, With<Text>>,
) {
    for tile_label in tile_label_query.iter() {
        if let Ok(mut visibility) = text_query.get_mut(tile_label.0) {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Generates tile position labels.
#[cfg(debug_assertions)]
fn spawn_tile_labels(
    mut commands: Commands,
    tilemap_query: Query<
        (&Transform, &TilemapType, &TilemapGridSize, &TileStorage),
        With<BaseTerrainLayer>,
    >,
    tile_query: Query<&mut TilePos>,
    font_handle: Res<FontHandle>,
) {
    let text_style = TextStyle {
        font: font_handle.clone(),
        font_size: 20.0,
        color: Color::BLACK,
    };
    let text_justify = JustifyText::Center;
    let (map_transform, map_type, grid_size, tilemap_storage) = tilemap_query.get_single().unwrap();
    for tile_entity in tilemap_storage.iter().flatten() {
        let tile_pos = tile_query.get(*tile_entity).unwrap();
        let tile_center = tile_pos
            .center_in_world(grid_size, map_type)
            .extend(TILE_LABEL_Z_INDEX);
        let transform = *map_transform * Transform::from_translation(tile_center);

        let label_entity = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    format!("{x}, {y}", x = tile_pos.x, y = tile_pos.y),
                    text_style.clone(),
                )
                .with_justify(text_justify),
                transform,
                ..Default::default()
            })
            .id();
        commands
            .entity(*tile_entity)
            .insert(TileLabel(label_entity));
    }
}

/// Keeps the cursor position updated based on any `CursorMoved` events.
fn update_cursor_pos(
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
#[cfg(debug_assertions)]
fn highlight_tile_labels(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    tilemap_query: Query<
        (
            &TilemapSize,
            &TilemapGridSize,
            &TilemapType,
            &TileStorage,
            &Transform,
        ),
        With<BaseTerrainLayer>,
    >,
    highlighted_tiles_query: Query<Entity, With<HighlightedLabel>>,
    tile_label_query: Query<&TileLabel>,
    mut text_query: Query<&mut Text>,
) {
    // Un-highlight any previously highlighted tile labels.
    for highlighted_tile_entity in highlighted_tiles_query.iter() {
        if let Ok(label) = tile_label_query.get(highlighted_tile_entity) {
            if let Ok(mut tile_text) = text_query.get_mut(label.0) {
                for section in tile_text.sections.iter_mut() {
                    section.style.color = Color::BLACK;
                }
                commands
                    .entity(highlighted_tile_entity)
                    .remove::<HighlightedLabel>();
            }
        }
    }

    let (map_size, grid_size, map_type, tile_storage, map_transform) =
        tilemap_query.get_single().unwrap();
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
        // Highlight the relevant tile's label
        if let Some(tile_entity) = tile_storage.get(&tile_pos) {
            if let Ok(label) = tile_label_query.get(tile_entity) {
                if let Ok(mut tile_text) = text_query.get_mut(label.0) {
                    for section in tile_text.sections.iter_mut() {
                        section.style.color = palettes::tailwind::RED_600.into();
                    }
                    commands.entity(tile_entity).insert(HighlightedLabel);
                }
            }
        }
    }
}

fn choose_base_terrain_by_latitude(rng: &mut fastrand::Rng, latitude: NotNan<f64>) -> BaseTerrain {
    if *latitude >= EarthLatitude::ArticCirle.latitude()
        || *latitude <= EarthLatitude::AntarcticCircle.latitude()
    {
        rng.choice(FRIGID_ZONE_TILE_CHOICES).unwrap()
    } else if *latitude >= 35.0 || *latitude <= -35.0 {
        rng.choice(TEMPERATE_ZONE_TILE_CHOICES).unwrap()
    } else if *latitude >= EarthLatitude::TropicOfCancer.latitude()
        || *latitude <= EarthLatitude::TropicOfCapricorn.latitude()
    {
        rng.choice(SUBTROPICS_TILE_CHOICES).unwrap()
    } else {
        rng.choice(TROPICS_TILE_CHOICES).unwrap()
    }
}
