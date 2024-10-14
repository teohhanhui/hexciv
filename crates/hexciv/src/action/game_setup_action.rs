use bevy::input::keyboard::KeyCode;
use bevy::reflect::Reflect;
use leafwing_input_manager::input_map::InputMap;
use leafwing_input_manager::Actionlike;

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
