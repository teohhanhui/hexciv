use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_pancam::PanCamPlugin;
use hexciv::action::{CursorAction, GameSetupAction, GlobalAction, UnitAction};
use hexciv::asset::FontHandle;
use hexciv::dev_tools::TileLabelPlugin;
use hexciv::game_setup::{host_game, join_game, GameSetupSet, HostingSet, InGameSet, JoiningSet};
use hexciv::input::{update_cursor_pos, update_cursor_tile_pos, CursorPos, CursorTilePos};
use hexciv::peer::{
    dispatch_host_broadcast, dispatch_request, handle_peer_connected, receive_host_broadcast,
    receive_request, send_host_broadcast, send_request, start_matchbox_socket, wait_for_peers,
    HostBroadcast, HostId, OurPeerId, PeerConnected, ReceiveHostBroadcastSet, ReceiveRequestSet,
    Request, SocketRxQueue,
};
use hexciv::player::{init_our_player, spawn_players};
use hexciv::state::{GameState, MultiplayerState, TurnState};
use hexciv::terrain::{post_spawn_tilemap, spawn_tilemap, upgrade_camera, SpawnTilemapSet};
use hexciv::turn::{handle_turn_started, TurnInProgressSet, TurnStarted};
use hexciv::unit::{
    cycle_ready_unit, focus_camera_on_active_unit, handle_unit_moved, handle_unit_selected,
    handle_unit_spawned, has_ready_units, mark_active_unit_fortified,
    mark_active_unit_out_of_orders, move_active_unit_to, reset_movement_points, select_unit,
    should_move_active_unit_to, spawn_starting_units, ActionsLegend, UnitEntityMap, UnitMoved,
    UnitSelected, UnitSpawned,
};
use leafwing_input_manager::common_conditions::action_just_pressed;
use leafwing_input_manager::prelude::*;

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
    .init_resource::<UnitEntityMap>()
    .init_resource::<CursorPos>()
    .insert_resource(ClearColor(Srgba::hex("#E9D4B1").unwrap().into()))
    .insert_resource(GameSetupAction::input_map())
    .insert_resource(GlobalAction::input_map())
    .insert_resource(UnitAction::input_map())
    .insert_resource(CursorAction::input_map())
    .init_state::<MultiplayerState>()
    .init_state::<GameState>()
    .add_sub_state::<TurnState>()
    .add_event::<HostBroadcast>()
    .add_event::<Request>()
    .add_event::<PeerConnected>()
    .add_event::<TurnStarted>()
    .add_event::<UnitSpawned>()
    .add_event::<UnitSelected>()
    .add_event::<UnitMoved>()
    .configure_sets(
        Update,
        (
            HostingSet.run_if(in_state(MultiplayerState::Hosting)),
            JoiningSet.run_if(in_state(MultiplayerState::Joining)),
            GameSetupSet.run_if(in_state(GameState::GameSetup)),
            InGameSet.run_if(in_state(GameState::InGame)),
            TurnInProgressSet.run_if(in_state(TurnState::InProgress)),
        ),
    )
    .add_systems(Startup, (setup, start_matchbox_socket))
    .add_systems(
        OnEnter(GameState::InGame),
        (spawn_tilemap, post_spawn_tilemap)
            .chain()
            .in_set(SpawnTilemapSet),
    )
    .add_systems(
        OnEnter(GameState::InGame),
        upgrade_camera.after(SpawnTilemapSet),
    )
    .add_systems(
        OnEnter(GameState::InGame),
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
        OnEnter(GameState::InGame),
        (
            spawn_players,
            init_our_player
                .after(ReceiveHostBroadcastSet)
                .after(handle_peer_connected),
            spawn_starting_units
                .after(SpawnTilemapSet)
                .run_if(in_state(MultiplayerState::Hosting)),
        )
            .chain(),
    )
    .add_systems(
        OnEnter(TurnState::InProgress),
        (
            reset_movement_points,
            cycle_ready_unit.before(handle_unit_selected),
            focus_camera_on_active_unit.after(handle_unit_selected),
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
            (
                send_host_broadcast,
                (
                    receive_request,
                    dispatch_request.run_if(on_event::<Request>()),
                )
                    .chain()
                    .in_set(ReceiveRequestSet),
            )
                .in_set(HostingSet),
            (
                send_request,
                (
                    receive_host_broadcast,
                    dispatch_host_broadcast.run_if(on_event::<HostBroadcast>()),
                )
                    .chain()
                    .in_set(ReceiveHostBroadcastSet),
            )
                .in_set(JoiningSet),
        )
            .run_if(resource_exists::<OurPeerId>.and_then(resource_exists::<HostId>)),
    )
    .add_systems(
        Update,
        (
            // TODO: Ensure events are processed in-order.
            handle_peer_connected.run_if(on_event::<PeerConnected>()),
            handle_turn_started.run_if(on_event::<TurnStarted>()),
            handle_unit_spawned.run_if(on_event::<UnitSpawned>()),
            handle_unit_moved.run_if(on_event::<UnitMoved>()),
        )
            .after(ReceiveHostBroadcastSet)
            .after(ReceiveRequestSet)
            .in_set(InGameSet),
    )
    .add_systems(
        Update,
        (
            cycle_ready_unit.before(handle_unit_selected),
            focus_camera_on_active_unit.after(handle_unit_selected),
        )
            .chain()
            .run_if(
                action_just_pressed(GlobalAction::PreviousReadyUnit)
                    .or_else(action_just_pressed(GlobalAction::NextReadyUnit))
                    .and_then(has_ready_units),
            )
            .in_set(TurnInProgressSet),
    )
    .add_systems(
        Update,
        (
            mark_active_unit_out_of_orders.run_if(action_just_pressed(UnitAction::SkipTurn)),
            mark_active_unit_fortified.run_if(action_just_pressed(UnitAction::Fortify)),
        )
            .in_set(TurnInProgressSet),
    )
    .add_systems(
        Update,
        (update_cursor_pos, update_cursor_tile_pos)
            .chain()
            .in_set(InGameSet),
    )
    .add_systems(
        Update,
        (
            select_unit
                .before(handle_unit_selected)
                .run_if(action_just_pressed(CursorAction::Click)),
            move_active_unit_to
                .run_if(
                    action_just_pressed(CursorAction::SecondaryClick)
                        .and_then(should_move_active_unit_to),
                )
                .in_set(TurnInProgressSet),
        )
            .after(update_cursor_tile_pos)
            .run_if(resource_exists::<CursorTilePos>)
            .in_set(InGameSet),
    )
    .add_systems(
        Update,
        handle_unit_selected
            .run_if(on_event::<UnitSelected>())
            .in_set(InGameSet),
    );

    #[cfg(debug_assertions)]
    {
        app.add_plugins(TileLabelPlugin);
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
