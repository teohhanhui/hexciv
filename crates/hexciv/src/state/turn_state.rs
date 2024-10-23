use bevy::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default, States)]
pub enum TurnState {
    #[default]
    Processing,
    Playing,
}
