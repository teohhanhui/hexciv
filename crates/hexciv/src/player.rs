use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;

use crate::civilization::Civilization;

#[derive(Component)]
pub struct Player;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default, Component)]
pub enum PlayerState {
    Playing,
    #[default]
    EndTurn,
}

#[derive(Bundle)]
pub struct PlayerBundle {
    player: Player,
    pub civ: Civilization,
    pub player_state: PlayerState,
}

impl PlayerBundle {
    pub fn new(civ: Civilization) -> Self {
        Self {
            player: Player,
            civ,
            player_state: PlayerState::default(),
        }
    }
}
