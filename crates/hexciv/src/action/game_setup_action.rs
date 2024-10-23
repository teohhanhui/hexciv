use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Actionlike, Reflect)]
pub enum GameSetupAction {
    HostGame,
    JoinGame,
}

impl GameSetupAction {
    pub fn input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();
        input_map.insert(Self::HostGame, KeyCode::KeyH);
        input_map.insert(Self::JoinGame, KeyCode::KeyJ);
        input_map
    }
}
