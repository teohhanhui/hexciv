use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::peer::HostBroadcast;
use crate::state::{MultiplayerState, TurnState};

#[derive(Eq, PartialEq, Debug, Resource)]
pub struct CurrentTurn(pub u16);

#[derive(Copy, Clone, Debug, Deserialize, Event, Serialize)]
pub struct TurnStarted {
    pub turn_num: u16,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct TurnInProgressSet;

pub fn mark_turn_in_progress(mut next_turn_state: ResMut<NextState<TurnState>>) {
    next_turn_state.set(TurnState::InProgress);
}

/// Handles [`TurnStarted`] events.
pub fn handle_turn_started(
    mut commands: Commands,
    mut current_turn: Option<ResMut<CurrentTurn>>,
    multiplayer_state: Res<State<MultiplayerState>>,
    mut host_broadcast_events: EventWriter<HostBroadcast>,
    mut turn_started_events: EventReader<TurnStarted>,
) {
    let mut new_current_turn = None;

    let mut updated = false;

    for &turn_started in turn_started_events.read() {
        debug!(?turn_started, "handling turn started");

        if let Some(current_turn) = &mut current_turn {
            current_turn.set_if_neq(CurrentTurn(turn_started.turn_num));
            updated = true;
        } else {
            new_current_turn = Some(CurrentTurn(turn_started.turn_num));
        }

        if matches!(multiplayer_state.get(), MultiplayerState::Hosting) {
            host_broadcast_events.send(turn_started.into());
        }
    }

    if !updated {
        if let Some(new_current_turn) = new_current_turn {
            commands.insert_resource(new_current_turn);
        }
    }
}
