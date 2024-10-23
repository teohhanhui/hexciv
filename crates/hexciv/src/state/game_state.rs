use bevy::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default, States)]
pub enum GameState {
    #[default]
    Setup,
    Playing,
}
