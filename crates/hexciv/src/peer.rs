use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;
use bevy_matchbox::prelude as matchbox;
use bevy_matchbox::prelude::*;
use serde::{Deserialize, Serialize};

use crate::state::MultiplayerState;
use crate::turn::TurnStarted;
use crate::unit::{UnitMoved, UnitSpawned};

#[derive(Debug, Resource)]
pub struct OurPeerId(pub matchbox::PeerId);

#[derive(Debug, Resource)]
pub struct HostId(pub matchbox::PeerId);

#[derive(Default, Resource)]
pub struct SocketRxQueue(pub VecDeque<(matchbox::PeerId, Box<[u8]>)>);

#[derive(Component)]
pub struct Peer;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Component)]
pub struct PeerId(pub matchbox::PeerId);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Component)]
pub struct PlayerSlotIndex(pub u8);

#[derive(Bundle)]
pub struct PeerBundle {
    peer: Peer,
    pub peer_id: PeerId,
    pub player_slot_index: PlayerSlotIndex,
}

#[derive(Copy, Clone, Debug, Deserialize, Event, Serialize)]
pub struct PeerConnected {
    pub peer_id: matchbox::PeerId,
    pub player_slot_index: u8,
}

#[derive(Debug, Deserialize, Event, Serialize)]
#[serde(untagged)]
pub enum HostBroadcast {
    PeerConnected(PeerConnected),
    TurnStarted(TurnStarted),
    UnitSpawned(UnitSpawned),
    UnitMoved(UnitMoved),
}

#[derive(Debug, Deserialize, Event, Serialize)]
#[serde(untagged)]
pub enum Request {
    UnitSpawned(UnitSpawned),
    UnitMoved(UnitMoved),
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct HostingSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct JoiningSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct ReceiveHostBroadcastSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct ReceiveRequestSet;

impl From<matchbox::PeerId> for PeerId {
    fn from(inner: matchbox::PeerId) -> Self {
        Self(inner)
    }
}

impl PeerBundle {
    pub fn new(peer_id: PeerId, player_slot_index: PlayerSlotIndex) -> Self {
        Self {
            peer: Peer,
            peer_id,
            player_slot_index,
        }
    }
}

impl From<PeerConnected> for HostBroadcast {
    fn from(inner: PeerConnected) -> Self {
        Self::PeerConnected(inner)
    }
}

impl From<TurnStarted> for HostBroadcast {
    fn from(inner: TurnStarted) -> Self {
        Self::TurnStarted(inner)
    }
}

impl From<UnitSpawned> for HostBroadcast {
    fn from(inner: UnitSpawned) -> Self {
        Self::UnitSpawned(inner)
    }
}

impl From<UnitMoved> for HostBroadcast {
    fn from(inner: UnitMoved) -> Self {
        Self::UnitMoved(inner)
    }
}

impl From<UnitSpawned> for Request {
    fn from(inner: UnitSpawned) -> Self {
        Self::UnitSpawned(inner)
    }
}

impl From<UnitMoved> for Request {
    fn from(inner: UnitMoved) -> Self {
        Self::UnitMoved(inner)
    }
}

pub fn start_matchbox_socket(mut commands: Commands) {
    let room_url = format!(
        "ws://{host}:{port}/hexciv?next=2",
        host = option_env!("MATCHBOX_HOST").unwrap_or("127.0.0.1"),
        port = option_env!("MATCHBOX_PORT").unwrap_or("3536")
    );
    info!(room_url, "connecting to matchbox server");
    commands.insert_resource(MatchboxSocket::new_reliable(room_url));
}

/// Sends [`HostBroadcast`] events to all connected peers.
///
/// This should be called on the host.
pub fn send_host_broadcast(
    mut socket: ResMut<MatchboxSocket<SingleChannel>>,
    host_id: Res<HostId>,
    our_peer_id: Res<OurPeerId>,
    mut host_broadcast_events: EventReader<HostBroadcast>,
) {
    assert!(our_peer_id.0 == host_id.0);
    let peers: Vec<_> = socket.connected_peers().collect();

    for host_broadcast in host_broadcast_events.read() {
        debug!(?host_broadcast, host_id = ?host_id.0, ?peers, "sending host broadcast");
        let message = serde_json::to_vec(host_broadcast)
            .expect("serializing host broadcast event should not fail");

        for &peer_id in &peers {
            socket.send(message.clone().into(), peer_id);
        }
    }
}

/// Receives [`HostBroadcast`] events from the host.
///
/// This should not be called on the host.
pub fn receive_host_broadcast(
    mut socket: ResMut<MatchboxSocket<SingleChannel>>,
    mut socket_rx_queue: ResMut<SocketRxQueue>,
    our_peer_id: Res<OurPeerId>,
    host_id: Res<HostId>,
    mut host_broadcast_events: EventWriter<HostBroadcast>,
) {
    assert!(our_peer_id.0 != host_id.0);
    socket_rx_queue.0.extend(socket.receive());

    for (peer_id, message) in socket_rx_queue.0.drain(..) {
        assert!(peer_id == host_id.0);
        let host_broadcast: HostBroadcast = serde_json::from_slice(&message)
            .expect("deserializing host broadcast event should not fail");
        debug!(?host_broadcast, host_id = ?host_id.0, our_peer_id = ?our_peer_id.0, "received host broadcast");
        host_broadcast_events.send(host_broadcast);
    }
}

/// Reads [`HostBroadcast`] events and dispatches the inner events to the
/// [`EventWriter<T>`] of their respective event types.
///
/// This should not be called on the host.
pub fn dispatch_host_broadcast(
    our_peer_id: Res<OurPeerId>,
    host_id: Res<HostId>,
    mut host_broadcast_events: EventReader<HostBroadcast>,
    mut peer_connected_events: EventWriter<PeerConnected>,
    mut turn_started_events: EventWriter<TurnStarted>,
    mut unit_spawned_events: EventWriter<UnitSpawned>,
    mut unit_moved_events: EventWriter<UnitMoved>,
) {
    assert!(our_peer_id.0 != host_id.0);
    for host_broadcast in host_broadcast_events.read() {
        match *host_broadcast {
            HostBroadcast::PeerConnected(peer_connected) => {
                peer_connected_events.send(peer_connected);
            },
            HostBroadcast::TurnStarted(turn_started) => {
                turn_started_events.send(turn_started);
            },
            HostBroadcast::UnitSpawned(unit_spawned) => {
                unit_spawned_events.send(unit_spawned);
            },
            HostBroadcast::UnitMoved(unit_moved) => {
                unit_moved_events.send(unit_moved);
            },
        }
    }
}

/// Sends [`Request`] events to the host.
///
/// This should not be called on the host.
pub fn send_request(
    mut socket: ResMut<MatchboxSocket<SingleChannel>>,
    host_id: Res<HostId>,
    our_peer_id: Res<OurPeerId>,
    mut request_events: EventReader<Request>,
) {
    assert!(our_peer_id.0 != host_id.0);
    for request in request_events.read() {
        debug!(?request, host_id = ?host_id.0, our_peer_id = ?our_peer_id.0, "sending request");
        let message =
            serde_json::to_vec(request).expect("serializing request event should not fail");
        socket.send(message.into(), host_id.0);
    }
}

/// Receives [`Request`] events from connected peers.
///
/// This should be called on the host.
pub fn receive_request(
    mut socket: ResMut<MatchboxSocket<SingleChannel>>,
    mut socket_rx_queue: ResMut<SocketRxQueue>,
    host_id: Res<HostId>,
    our_peer_id: Res<OurPeerId>,
    mut request_events: EventWriter<Request>,
) {
    assert!(our_peer_id.0 == host_id.0);
    socket_rx_queue.0.extend(socket.receive());

    for (peer_id, message) in socket_rx_queue.0.drain(..) {
        assert!(peer_id != host_id.0);
        let request: Request =
            serde_json::from_slice(&message).expect("deserializing request event should not fail");
        debug!(?request, their_peer_id = ?peer_id, our_peer_id = ?our_peer_id.0, "received request");
        request_events.send(request);
    }
}

/// Reads [`Request`] events and dispatches the inner events to the
/// [`EventWriter<T>`] of their respective event types.
///
/// This should be called on the host.
pub fn dispatch_request(
    our_peer_id: Res<OurPeerId>,
    host_id: Res<HostId>,
    mut request_events: EventReader<Request>,
    mut unit_spawned_events: EventWriter<UnitSpawned>,
    mut unit_moved_events: EventWriter<UnitMoved>,
) {
    assert!(our_peer_id.0 == host_id.0);
    for request in request_events.read() {
        match *request {
            Request::UnitSpawned(unit_spawned) => {
                unit_spawned_events.send(unit_spawned);
            },
            Request::UnitMoved(unit_moved) => {
                unit_moved_events.send(unit_moved);
            },
        }
    }
}

/// Handles [`PeerConnected`] events.
pub fn handle_peer_connected(
    mut commands: Commands,
    multiplayer_state: Res<State<MultiplayerState>>,
    mut peer_query: Query<(&mut PeerId, &PlayerSlotIndex), With<Peer>>,
    mut host_broadcast_events: EventWriter<HostBroadcast>,
    mut peer_connected_events: EventReader<PeerConnected>,
) {
    let mut new_peer_bundles = HashMap::new();

    for &peer_connected in peer_connected_events.read() {
        debug!(?peer_connected, "handling peer connected");
        let PeerConnected {
            peer_id: connected_peer_id,
            player_slot_index: connected_player_slot_index,
        } = peer_connected;

        let mut updated = false;

        for (mut peer_id, player_slot_index) in peer_query.iter_mut() {
            if matches!(player_slot_index, &PlayerSlotIndex(s) if s == connected_player_slot_index)
            {
                peer_id.set_if_neq(PeerId(connected_peer_id));
                updated = true;
                break;
            }
        }

        if !updated {
            new_peer_bundles.insert(
                PlayerSlotIndex(connected_player_slot_index),
                PeerBundle::new(
                    PeerId(connected_peer_id),
                    PlayerSlotIndex(connected_player_slot_index),
                ),
            );
        }

        if matches!(multiplayer_state.get(), MultiplayerState::Hosting) {
            host_broadcast_events.send(peer_connected.into());
        }
    }

    commands.spawn_batch(new_peer_bundles.into_values());
}
