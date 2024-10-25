use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::peer::HostBroadcast;
use crate::state::{MultiplayerState, TurnState};

#[derive(Copy, Clone, Debug, Deserialize, Event, Serialize)]
pub struct TurnStarted {
    pub turn_num: u16,
}

/// Handles [`TurnStarted`] events.
pub fn handle_turn_started(
    multiplayer_state: Res<State<MultiplayerState>>,
    mut next_turn_state: ResMut<NextState<TurnState>>,
    mut host_broadcast_events: EventWriter<HostBroadcast>,
    mut turn_started_events: EventReader<TurnStarted>,
) {
    for &turn_started in turn_started_events.read() {
        debug!(?turn_started, "handling turn started");

        next_turn_state.set(TurnState::InProgress);

        if matches!(multiplayer_state.get(), MultiplayerState::Hosting) {
            host_broadcast_events.send(turn_started.into());
        }
    }
}
