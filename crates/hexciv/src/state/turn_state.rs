use bevy::prelude::*;

use super::GameState;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default, SubStates)]
#[source(GameState = GameState::InGame)]
pub enum TurnState {
    #[default]
    Processing,
    InProgress,
}
