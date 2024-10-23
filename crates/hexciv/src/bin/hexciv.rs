use std::any::TypeId;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter;
use std::ops::Add;

use bevy::color::palettes;
use bevy::ecs::system::{RunSystemOnce as _, SystemState};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_ecs_tilemap::helpers::hex_grid::cube::CubePos;
use bevy_ecs_tilemap::helpers::hex_grid::neighbors::{HexNeighbors, HEX_DIRECTIONS};
use bevy_ecs_tilemap::prelude::*;
use bevy_matchbox::prelude::*;
use bevy_pancam::{DirectionKeys, PanCam, PanCamPlugin};
use bitvec::prelude::*;
use fastlem_random_terrain::{generate_terrain, Site2D, Terrain2D};
use fastrand_contrib::RngExt as _;
#[cfg(debug_assertions)]
use hexciv::action::DebugAction;
use hexciv::action::{CursorAction, GameSetupAction, GlobalAction, UnitAction};
use hexciv::civilization::Civilization;
use hexciv::game_setup::{GameRng, GameSetup, MapRng};
use hexciv::layer::{
    BaseTerrainLayer, BaseTerrainLayerFilter, CivilianUnitLayer, CivilianUnitLayerFilter,
    LandMilitaryUnitLayer, LandMilitaryUnitLayerFilter, RiverLayer, RiverLayerFilter,
    TerrainFeaturesLayer, TerrainFeaturesLayerFilter, UnitLayersFilter, UnitSelectionLayer,
    UnitSelectionLayerFilter, UnitStateLayer, UnitStateLayerFilter,
};
use hexciv::peer::{
    dispatch_host_broadcast, handle_peer_connected, receive_host_broadcast, send_host_broadcast,
    start_matchbox_socket, HostBroadcast, HostId, HostingSet, JoiningSet, NetworkEntityMap,
    OurPeerId, PeerConnected, ReceiveHostBroadcastSet, SocketRxQueue,
};
use hexciv::player::{init_our_player, spawn_players, OurPlayer, Player};
use hexciv::state::{GameState, MultiplayerState, TurnState};
use hexciv::unit::{
    handle_unit_spawned, CivilianUnit, CivilianUnitTileBundle, FullMovementPoints,
    LandMilitaryUnit, LandMilitaryUnitTileBundle, MovementPoints, UnitFilter, UnitId, UnitMoved,
    UnitSelected, UnitSelection, UnitSelectionTileBundle, UnitSpawned, UnitState,
    UnitStateModifier, UnitStateTileBundle, UnitTileBundle, UnitType,
};
use indexmap::IndexSet;
use itertools::{chain, repeat_n, Itertools as _};
use leafwing_input_manager::common_conditions::{action_just_pressed, action_toggle_active};
use leafwing_input_manager::prelude::*;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use ordered_float::NotNan;
use pathfinding::directed::astar::astar;
use uuid::Uuid;

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

#[derive(Copy, Clone, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
enum BaseTerrain {
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
enum BaseTerrainVariant {
    Hills = 5,
    Mountains = 10,
}

type RiverEdges = BitArr!(for 6, in u32, Lsb0);

#[derive(Copy, Clone, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
enum TerrainFeatures {
    Woods = 0,
    Rainforest = 1,
    Marsh = 2,
    Floodplains = 3,
    Oasis = 4,
    Cliffs = 5,
    Ice = 6,
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
struct MapTerrain(Terrain2D);

#[derive(Resource)]
struct CursorPos(Vec2);

#[derive(Resource)]
struct CursorTilePos(TilePos);

#[derive(Component)]
struct ActionsLegend;

#[derive(Component)]
struct TileLabel(Entity);

#[derive(Component)]
struct HighlightedLabel;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
struct GameSetupSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
struct GamePlayingSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
struct TurnPlayingSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
struct SpawnTilemapSet;

trait LayerZIndex {
    const Z_INDEX: f32;
}

impl BaseTerrain {
    const HILLS: [Self; 5] = [
        Self::PlainsHills,
        Self::GrasslandHills,
        Self::DesertHills,
        Self::TundraHills,
        Self::SnowHills,
    ];
    const MOUNTAINS: [Self; 5] = [
        Self::PlainsMountains,
        Self::GrasslandMountains,
        Self::DesertMountains,
        Self::TundraMountains,
        Self::SnowMountains,
    ];

    fn is_hills(&self) -> bool {
        Self::HILLS.contains(self)
    }

    fn is_mountains(&self) -> bool {
        Self::MOUNTAINS.contains(self)
    }
}

impl Add<BaseTerrainVariant> for BaseTerrain {
    type Output = Self;

    fn add(self, rhs: BaseTerrainVariant) -> Self::Output {
        match self {
            BaseTerrain::Plains
            | BaseTerrain::Grassland
            | BaseTerrain::Desert
            | BaseTerrain::Tundra
            | BaseTerrain::Snow => {
                let base: u32 = self.into();
                let variant: u32 = rhs.into();
                Self::try_from(base + variant).unwrap()
            },
            BaseTerrain::PlainsHills
            | BaseTerrain::GrasslandHills
            | BaseTerrain::DesertHills
            | BaseTerrain::TundraHills
            | BaseTerrain::SnowHills
            | BaseTerrain::PlainsMountains
            | BaseTerrain::GrasslandMountains
            | BaseTerrain::DesertMountains
            | BaseTerrain::TundraMountains
            | BaseTerrain::SnowMountains => {
                unimplemented!("base terrain variants are not stackable");
            },
            BaseTerrain::Coast | BaseTerrain::Ocean => {
                unimplemented!("coast and ocean base terrain do not have variants");
            },
        }
    }
}

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
        Self(asset_server.load("fonts/NotoSansMono/NotoSansMono-Regular.ttf"))
    }
}

impl Default for CursorPos {
    fn default() -> Self {
        // Initialize the cursor pos at some far away place. It will get updated
        // correctly when the cursor moves.
        Self(Vec2::new(-1000.0, -1000.0))
    }
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

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Hexciv".to_owned(),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
                    ..Default::default()
                }),
                ..Default::default()
            })
            .set(ImagePlugin::default_nearest())
            .set({
                #[cfg(debug_assertions)]
                {
                    LogPlugin {
                        level: bevy::log::Level::DEBUG,
                        filter: "info,wgpu=error,naga=warn,hexciv=debug".to_owned(),
                        ..Default::default()
                    }
                }
                #[cfg(not(debug_assertions))]
                {
                    LogPlugin {
                        level: bevy::log::Level::WARN,
                        filter: "warn,wgpu=error,naga=warn,hexciv=warn".to_owned(),
                        ..Default::default()
                    }
                }
            }),
    )
    .add_plugins((
        InputManagerPlugin::<GameSetupAction>::default(),
        InputManagerPlugin::<GlobalAction>::default(),
        InputManagerPlugin::<UnitAction>::default(),
        InputManagerPlugin::<CursorAction>::default(),
    ))
    .add_plugins(PanCamPlugin)
    .add_plugins(TilemapPlugin)
    .init_resource::<FontHandle>()
    .init_resource::<ActionState<GameSetupAction>>()
    .init_resource::<ActionState<GlobalAction>>()
    .init_resource::<ActionState<UnitAction>>()
    .init_resource::<ActionState<CursorAction>>()
    .init_resource::<SocketRxQueue>()
    .init_resource::<NetworkEntityMap>()
    .init_resource::<CursorPos>()
    .insert_resource(ClearColor(Srgba::hex("#E9D4B1").unwrap().into()))
    .insert_resource(GameSetupAction::input_map())
    .insert_resource(GlobalAction::input_map())
    .insert_resource(UnitAction::input_map())
    .insert_resource(CursorAction::input_map())
    .init_state::<MultiplayerState>()
    .init_state::<GameState>()
    .init_state::<TurnState>()
    .add_event::<HostBroadcast>()
    .add_event::<PeerConnected>()
    .add_event::<UnitSpawned>()
    .add_event::<UnitSelected>()
    .add_event::<UnitMoved>()
    .configure_sets(
        Update,
        (
            HostingSet.run_if(in_state(MultiplayerState::Hosting)),
            JoiningSet.run_if(in_state(MultiplayerState::Joining)),
            GameSetupSet.run_if(in_state(GameState::Setup)),
            GamePlayingSet.run_if(in_state(GameState::Playing)),
            TurnPlayingSet.run_if(in_state(TurnState::Playing)),
        ),
    )
    .add_systems(Startup, (setup, start_matchbox_socket))
    .add_systems(
        OnEnter(GameState::Playing),
        (spawn_tilemap, post_spawn_tilemap)
            .chain()
            .in_set(SpawnTilemapSet),
    )
    .add_systems(
        OnEnter(GameState::Playing),
        upgrade_camera.after(SpawnTilemapSet),
    )
    .add_systems(
        OnEnter(GameState::Playing),
        (
            (receive_host_broadcast, dispatch_host_broadcast)
                .chain()
                .run_if(in_state(MultiplayerState::Joining))
                .in_set(ReceiveHostBroadcastSet),
            handle_peer_connected,
        )
            .chain(),
    )
    .add_systems(
        OnEnter(GameState::Playing),
        (
            spawn_players,
            init_our_player
                .after(ReceiveHostBroadcastSet)
                .after(handle_peer_connected),
            spawn_starting_units
                .after(SpawnTilemapSet)
                .after(upgrade_camera)
                .run_if(in_state(MultiplayerState::Hosting)),
        )
            .chain(),
    )
    .add_systems(
        OnEnter(TurnState::Playing),
        (
            reset_movement_points,
            cycle_ready_unit,
            sync_unit_selected,
            focus_camera_on_active_unit,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            (
                host_game.run_if(action_just_pressed(GameSetupAction::HostGame)),
                join_game.run_if(action_just_pressed(GameSetupAction::JoinGame)),
            )
                .run_if(in_state(MultiplayerState::Inactive)),
            wait_for_peers
                .before(send_host_broadcast)
                .before(ReceiveHostBroadcastSet)
                .before(handle_peer_connected)
                .run_if(
                    in_state(MultiplayerState::Hosting)
                        .or_else(in_state(MultiplayerState::Joining)),
                ),
        )
            .in_set(GameSetupSet),
    )
    .add_systems(
        Update,
        (
            send_host_broadcast
                .run_if(resource_exists::<OurPeerId>.and_then(resource_exists::<HostId>))
                .in_set(HostingSet),
            (receive_host_broadcast, dispatch_host_broadcast)
                .chain()
                .run_if(resource_exists::<OurPeerId>.and_then(resource_exists::<HostId>))
                .in_set(JoiningSet)
                .in_set(ReceiveHostBroadcastSet),
        ),
    )
    .add_systems(
        Update,
        (handle_peer_connected, handle_unit_spawned)
            .after(ReceiveHostBroadcastSet)
            .in_set(GamePlayingSet),
    )
    .add_systems(
        Update,
        (
            cycle_ready_unit,
            sync_unit_selected,
            focus_camera_on_active_unit,
        )
            .chain()
            .run_if(
                action_just_pressed(GlobalAction::PreviousReadyUnit)
                    .or_else(action_just_pressed(GlobalAction::NextReadyUnit))
                    .and_then(has_ready_units),
            )
            .in_set(GamePlayingSet)
            .in_set(TurnPlayingSet),
    )
    .add_systems(
        Update,
        (
            mark_active_unit_out_of_orders.run_if(action_just_pressed(UnitAction::SkipTurn)),
            mark_active_unit_fortified.run_if(action_just_pressed(UnitAction::Fortify)),
        )
            .in_set(GamePlayingSet)
            .in_set(TurnPlayingSet),
    )
    .add_systems(
        Update,
        (update_cursor_pos, update_cursor_tile_pos)
            .chain()
            .in_set(GamePlayingSet),
    )
    .add_systems(
        Update,
        (
            (select_unit, sync_unit_selected)
                .chain()
                .run_if(action_just_pressed(CursorAction::Click)),
            (move_active_unit_to, sync_unit_moved)
                .chain()
                .run_if(
                    action_just_pressed(CursorAction::SecondaryClick)
                        .and_then(should_move_active_unit_to),
                )
                .in_set(TurnPlayingSet),
        )
            .after(update_cursor_tile_pos)
            .run_if(resource_exists::<CursorTilePos>)
            .in_set(GamePlayingSet),
    );

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
                    highlight_tile_labels.after(update_cursor_tile_pos),
                )
                    .chain()
                    .in_set(GamePlayingSet),
            );
    }

    app.run();
}

fn setup(mut commands: Commands, font_handle: Res<FontHandle>) {
    commands.spawn(Camera2dBundle::default());
    commands
        .spawn(
            TextBundle::from_section("[H] Host game\n[J] Join game", TextStyle {
                font: font_handle.clone(),
                font_size: 24.0,
                color: Srgba::hex("#5C3F21").unwrap().into(),
            })
            .with_style(Style {
                position_type: PositionType::Absolute,
                top: Val::Px(12.),
                left: Val::Px(12.),
                ..Default::default()
            }),
        )
        .insert(ActionsLegend);
}

fn host_game(
    mut commands: Commands,
    mut actions_legend_text_query: Query<(&mut Text,), With<ActionsLegend>>,
    mut next_multiplayer_state: ResMut<NextState<MultiplayerState>>,
) {
    let (mut actions_legend_text,) = actions_legend_text_query.get_single_mut().unwrap();

    actions_legend_text.sections[0].value = "Hosting game...\n".to_owned();

    commands.insert_resource(MapRng(fastrand::Rng::new()));
    commands.insert_resource(GameRng(fastrand::Rng::new()));

    next_multiplayer_state.set(MultiplayerState::Hosting);
}

fn join_game(
    mut actions_legend_text_query: Query<(&mut Text,), With<ActionsLegend>>,
    mut next_multiplayer_state: ResMut<NextState<MultiplayerState>>,
) {
    let (mut actions_legend_text,) = actions_legend_text_query.get_single_mut().unwrap();

    actions_legend_text.sections[0].value = "Joining game...\n".to_owned();

    next_multiplayer_state.set(MultiplayerState::Joining);
}

#[allow(clippy::too_many_arguments)]
fn wait_for_peers(
    mut commands: Commands,
    mut socket: ResMut<MatchboxSocket<SingleChannel>>,
    mut socket_rx_queue: ResMut<SocketRxQueue>,
    map_rng: Option<Res<MapRng>>,
    game_rng: Option<Res<GameRng>>,
    mut actions_legend_text_query: Query<(&mut Text,), With<ActionsLegend>>,
    multiplayer_state: Res<State<MultiplayerState>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut host_broadcast_events: EventWriter<HostBroadcast>,
    mut peer_connected_events: EventWriter<PeerConnected>,
) {
    let (mut actions_legend_text,) = actions_legend_text_query.get_single_mut().unwrap();

    // Check for new connections.
    socket.update_peers();

    let peers: Vec<_> = socket.connected_peers().collect();
    let num_players: u8 = 2;
    if peers.len() < (num_players - 1).into() {
        // Keep waiting until all peers have connected.
        let msg = "Waiting for peers...\n";
        if !actions_legend_text.sections[0].value.ends_with(msg) {
            actions_legend_text.sections[0].value += msg;
        }
        return;
    }

    info!("all peers have connected");

    let our_peer_id = socket
        .id()
        .expect("our peer should have an assigned peer id");

    let host_id = match multiplayer_state.get() {
        MultiplayerState::Hosting => {
            let host_id = our_peer_id;
            let game_setup = GameSetup {
                map_seed: map_rng.expect("map_rng should not be None").0.get_seed(),
                game_seed: game_rng.expect("game_rng should not be None").0.get_seed(),
                num_players,
            };
            debug!(
                ?game_setup,
                ?host_id,
                ?peers,
                "sending broadcast of game setup from host"
            );
            let game_setup_message =
                serde_json::to_vec(&game_setup).expect("serializing game setup should not fail");
            for &peer_id in &peers {
                socket.send(game_setup_message.clone().into(), peer_id);
            }
            for peer_connected in
                iter::once(host_id)
                    .chain(peers)
                    .enumerate()
                    .map(|(i, peer_id)| PeerConnected {
                        peer_id,
                        player_slot_index: i.try_into().unwrap(),
                    })
            {
                host_broadcast_events.send(HostBroadcast::PeerConnected(peer_connected));
                // Send the peer connected event to be used on the host itself.
                peer_connected_events.send(peer_connected);
            }
            host_id
        },
        MultiplayerState::Joining => {
            socket_rx_queue.0.extend(socket.receive());
            let Some((host_id, game_setup_message)) = socket_rx_queue.0.front() else {
                // Keep waiting for game setup messsage.
                return;
            };
            let game_setup = serde_json::from_slice(game_setup_message)
                .expect("deserializing game setup should not fail");
            debug!(?game_setup, ?host_id, "received game setup");
            let GameSetup {
                map_seed,
                game_seed,
                num_players,
            } = game_setup;
            if socket_rx_queue.0.len() < (num_players + 1).into() {
                // Keep waiting for peer connected event messages.
                return;
            }
            let (host_id, _) = socket_rx_queue.0.pop_front().unwrap();
            commands.insert_resource(MapRng(fastrand::Rng::with_seed(map_seed)));
            commands.insert_resource(GameRng(fastrand::Rng::with_seed(game_seed)));
            host_id
        },
        _ => {
            unreachable!("multiplayer state should not be inactive");
        },
    };

    commands.insert_resource(OurPeerId(our_peer_id));
    commands.insert_resource(HostId(host_id));

    next_game_state.set(GameState::Playing);
}

/// Generates the initial tilemap.
fn spawn_tilemap(
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
            let elevation = elevations.iter().sum::<NotNan<_>>() / elevations.len() as f64;
            let texture_index = if elevation < NotNan::new(0.05).unwrap() {
                TileTextureIndex(BaseTerrain::Ocean.into())
            } else {
                let latitude = NotNan::new(-90.0).unwrap()
                    + NotNan::new(180.0).unwrap()
                        * ((NotNan::from(tile_pos.y) + 0.5) / NotNan::from(map_size.y));

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
fn post_spawn_tilemap(
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

            if [BaseTerrain::Ocean.into(), BaseTerrain::Coast.into()].contains(&tile_texture.0) {
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
                            texture_index: TileTextureIndex(TerrainFeatures::Ice.into()),
                            ..Default::default()
                        })
                        .insert(TerrainFeaturesLayer)
                        .id();
                    terrain_features_tile_storage.set(&tile_pos, tile_entity);
                }
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

fn upgrade_camera(mut commands: Commands, camera_query: Query<(Entity,), With<Camera2d>>) {
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

fn spawn_starting_units(
    mut next_turn_state: ResMut<NextState<TurnState>>,
    mut game_rng: ResMut<GameRng>,
    player_query: Query<(&Civilization,), With<Player>>,
    base_terrain_tilemap_query: Query<(&TilemapSize, &TileStorage), BaseTerrainLayerFilter>,
    base_terrain_tile_query: Query<(&TileTextureIndex,), BaseTerrainLayerFilter>,
    mut host_broadcast_events: EventWriter<HostBroadcast>,
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

    for (&civ,) in player_query.iter() {
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
        let unit_spawned = UnitSpawned {
            network_id: Uuid::new_v4().into(),
            position: settler_tile_pos,
            unit_type: CivilianUnit::Settler.into(),
            civ,
        };
        host_broadcast_events.send(HostBroadcast::UnitSpawned(unit_spawned));
        unit_spawned_events.send(unit_spawned);

        // Spawn warrior.
        let unit_spawned = UnitSpawned {
            network_id: Uuid::new_v4().into(),
            position: warrior_tile_pos,
            unit_type: LandMilitaryUnit::Warrior.into(),
            civ,
        };
        host_broadcast_events.send(HostBroadcast::UnitSpawned(unit_spawned));
        unit_spawned_events.send(unit_spawned);
    }

    next_turn_state.set(TurnState::Playing);
}

/// Resets movement points for all units.
fn reset_movement_points(
    mut unit_query: Query<(&mut MovementPoints, &FullMovementPoints), UnitFilter>,
) {
    for (mut movement_points, full_movement_points) in unit_query.iter_mut() {
        movement_points.0 = full_movement_points.0;
    }
}

/// Checks if there are ready units controlled by the current player.
fn has_ready_units(
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
fn cycle_ready_unit(
    our_player: Res<OurPlayer>,
    global_action_state: Res<ActionState<GlobalAction>>,
    player_query: Query<(&Civilization,), With<Player>>,
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex, &UnitId),
        UnitSelectionLayerFilter,
    >,
    unit_query: Query<(Entity, &TilePos, &Civilization, &UnitState), UnitFilter>,
    mut unit_selected_events: EventWriter<UnitSelected>,
) {
    let (current_civ,) = player_query.get(our_player.0).unwrap();

    let ready_units: IndexSet<_> = unit_query
        .iter()
        .filter_map(|(unit_entity, tile_pos, civ, unit_state)| {
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
            .find(|(_tile_pos, &tile_texture, _unit_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });

    if let Some((_active_unit_tile_pos, _tile_texture, UnitId(active_unit_entity))) =
        active_unit_selection
    {
        // Select the previous / next ready unit.

        let units: Vec<_> = unit_query
            .iter()
            .filter_map(|(unit_entity, &tile_pos, civ, _unit_state)| {
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

fn focus_camera_on_active_unit(
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

fn mark_active_unit_out_of_orders(
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex, &UnitId),
        UnitSelectionLayerFilter,
    >,
    mut unit_state_tile_query: Query<(&mut TileTextureIndex,), UnitStateLayerFilter>,
    mut unit_query: Query<(&mut UnitState,), UnitFilter>,
) {
    let (unit_state_tile_storage,) = unit_state_tilemap_query.get_single().unwrap();

    let active_unit_selection =
        unit_selection_tile_query
            .iter()
            .find(|(_tile_pos, &tile_texture, _unit_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });
    let Some((active_unit_tile_pos, _tile_texture, &UnitId(active_unit_entity))) =
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

fn mark_active_unit_fortified(
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    land_military_unit_tilemap_query: Query<(&TileStorage,), LandMilitaryUnitLayerFilter>,
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex, &UnitId),
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
            .find(|(_tile_pos, &tile_texture, _unit_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });
    let Some((active_unit_tile_pos, _tile_texture, &UnitId(active_unit_entity))) =
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

/// Keeps the cursor position updated based on any [`CursorMoved`] events.
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
fn update_cursor_tile_pos(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    tilemap_query: Query<
        (&TilemapSize, &TilemapGridSize, &TilemapType, &Transform),
        BaseTerrainLayerFilter,
    >,
) {
    let (map_size, grid_size, map_type, map_transform) = tilemap_query.get_single().unwrap();
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
        commands.insert_resource(CursorTilePos(tile_pos));
    } else {
        // Cursor is not hovering over any tile.
        commands.remove_resource::<CursorTilePos>();
    }
}

/// Selects the unit at the cursor's tile position controlled by the current
/// player.
#[allow(clippy::too_many_arguments)]
fn select_unit(
    our_player: Res<OurPlayer>,
    cursor_tile_pos: Res<CursorTilePos>,
    player_query: Query<(&Civilization,), With<Player>>,
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    unit_selection_tile_query: Query<
        (&TilePos, &TileTextureIndex, &UnitId),
        UnitSelectionLayerFilter,
    >,
    unit_state_tile_query: Query<(&UnitId,), UnitStateLayerFilter>,
    unit_query: Query<(Entity, &TilePos, &Civilization), UnitFilter>,
    mut unit_selected_events: EventWriter<UnitSelected>,
) {
    let (unit_state_tile_storage,) = unit_state_tilemap_query.get_single().unwrap();

    let (&current_civ,) = player_query.get(our_player.0).unwrap();

    let units: Vec<_> = unit_query
        .iter()
        .filter_map(|(unit_entity, &tile_pos, &civ)| {
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
            .find(|(_tile_pos, &tile_texture, _unit_id)| {
                matches!(tile_texture, TileTextureIndex(t) if t == u32::from(UnitSelection::Active))
            });

    if let Some((active_unit_tile_pos, _tile_texture, UnitId(active_unit_entity))) =
        active_unit_selection
            .filter(|(&tile_pos, _tile_texture, _unit_id)| tile_pos == cursor_tile_pos.0)
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
        let (&UnitId(unit_entity),) = unit_state_tile_query.get(tile_entity).unwrap();

        unit_selected_events.send(UnitSelected {
            entity: unit_entity,
            position: cursor_tile_pos.0,
        });
    }
}

fn should_move_active_unit_to(
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
fn move_active_unit_to(
    cursor_tile_pos: Res<CursorTilePos>,
    base_terrain_tilemap_query: Query<(&TilemapSize, &TileStorage), BaseTerrainLayerFilter>,
    river_tilemap_query: Query<(&TileStorage,), RiverLayerFilter>,
    terrain_features_tilemap_query: Query<(&TileStorage,), TerrainFeaturesLayerFilter>,
    unit_state_tilemap_query: Query<(&TileStorage,), UnitStateLayerFilter>,
    base_terrain_tile_query: Query<(&TileTextureIndex,), BaseTerrainLayerFilter>,
    river_tile_query: Query<(&TileTextureIndex,), RiverLayerFilter>,
    terrain_features_tile_query: Query<(&TileTextureIndex,), TerrainFeaturesLayerFilter>,
    unit_selection_tile_query: Query<(&TilePos, &TileTextureIndex), UnitSelectionLayerFilter>,
    unit_state_tile_query: Query<(&UnitId,), UnitStateLayerFilter>,
    unit_query: Query<(&MovementPoints, &FullMovementPoints), UnitFilter>,
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
    let (&UnitId(unit_entity),) = unit_state_tile_storage
        .get(&start)
        .map(|tile_entity| unit_state_tile_query.get(tile_entity).unwrap())
        .expect("active unit tile position should have unit state tile");
    let (movement_points, full_movement_points) = unit_query.get(unit_entity).unwrap();

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
            unit_moved_events.send(UnitMoved {
                entity: unit_entity,
                from_pos: current,
                to_pos: next,
                movement_cost,
            });
            current = next;
        } else {
            info!(?current, ?start, ?goal, "could not find path");
            // TODO: Show indication that there is no path for this move.
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_unit_selected(
    mut commands: Commands,
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
        debug!(?unit_selected, "unit selected");
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
                unit_actions_msg += "[F] Fortify\n";
            },
        }
        // Remove other unit tiles at the same tile position.
        for mut tile_storage in unit_tile_storages.into_values() {
            if let Some(tile_entity) = tile_storage.get(selected_unit_tile_pos) {
                commands.entity(tile_entity).despawn();
                tile_storage.remove(selected_unit_tile_pos);
            }
        }

        unit_actions_msg += "[Space] Skip Turn\n";
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
fn sync_unit_moved(
    mut commands: Commands,
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
        debug!(?unit_moved, "unit moved");
        let UnitMoved {
            entity: moved_unit_entity,
            from_pos,
            to_pos,
            movement_cost,
        } = unit_moved;

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

/// Generates tile position labels.
#[cfg(debug_assertions)]
fn spawn_tile_labels(
    mut commands: Commands,
    base_terrain_tilemap_query: Query<
        (&Transform, &TilemapType, &TilemapGridSize, &TileStorage),
        BaseTerrainLayerFilter,
    >,
    base_terrain_tile_query: Query<(&mut TilePos,), BaseTerrainLayerFilter>,
    font_handle: Res<FontHandle>,
) {
    let text_style = TextStyle {
        font: font_handle.clone(),
        font_size: 20.0,
        color: Color::BLACK,
    };
    let text_justify = JustifyText::Center;
    let (map_transform, map_type, grid_size, tilemap_storage) =
        base_terrain_tilemap_query.get_single().unwrap();
    for tile_entity in tilemap_storage.iter().flatten() {
        let (tile_pos,) = base_terrain_tile_query.get(*tile_entity).unwrap();
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

#[cfg(debug_assertions)]
#[allow(clippy::type_complexity)]
fn show_tile_labels(
    world: &mut World,
    tile_label_query: &mut QueryState<(), With<TileLabel>>,
    system_state: &mut SystemState<(Query<(&TileLabel,)>, Query<(&mut Visibility,), With<Text>>)>,
) {
    if tile_label_query.iter(world).next().is_none() {
        world.run_system_once(spawn_tile_labels);
    }

    {
        let (tile_label_query, mut text_query) = system_state.get_mut(world);

        for (tile_label,) in tile_label_query.iter() {
            if let Ok((mut visibility,)) = text_query.get_mut(tile_label.0) {
                *visibility = Visibility::Visible;
            }
        }
    }
}

#[cfg(debug_assertions)]
fn hide_tile_labels(
    tile_label_query: Query<(&TileLabel,)>,
    mut text_query: Query<(&mut Visibility,), With<Text>>,
) {
    for (tile_label,) in tile_label_query.iter() {
        if let Ok((mut visibility,)) = text_query.get_mut(tile_label.0) {
            *visibility = Visibility::Hidden;
        }
    }
}

/// Highlights the tile position label under the cursor.
#[cfg(debug_assertions)]
fn highlight_tile_labels(
    mut commands: Commands,
    cursor_tile_pos: Option<Res<CursorTilePos>>,
    base_terrain_tilemap_query: Query<(&TileStorage,), BaseTerrainLayerFilter>,
    highlighted_base_terrain_tile_query: Query<(Entity,), With<HighlightedLabel>>,
    tile_label_query: Query<(&TileLabel,)>,
    mut text_query: Query<(&mut Text,)>,
) {
    // Un-highlight any previously highlighted tile labels.
    for (tile_entity,) in highlighted_base_terrain_tile_query.iter() {
        if let Ok((label,)) = tile_label_query.get(tile_entity) {
            if let Ok((mut tile_text,)) = text_query.get_mut(label.0) {
                for section in tile_text.sections.iter_mut() {
                    section.style.color = Color::BLACK;
                }
                commands.entity(tile_entity).remove::<HighlightedLabel>();
            }
        }
    }

    let (tile_storage,) = base_terrain_tilemap_query.get_single().unwrap();
    if let Some(cursor_tile_pos) = cursor_tile_pos {
        // Highlight the relevant tile's label
        if let Some(tile_entity) = tile_storage.get(&cursor_tile_pos.0) {
            if let Ok((label,)) = tile_label_query.get(tile_entity) {
                if let Ok((mut tile_text,)) = text_query.get_mut(label.0) {
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
