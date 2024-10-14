use bevy::input::keyboard::KeyCode;
use bevy::reflect::Reflect;
use leafwing_input_manager::input_map::InputMap;
use leafwing_input_manager::user_input::ModifierKey;
use leafwing_input_manager::Actionlike;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Actionlike, Reflect)]
pub enum DebugAction {
    ShowTileLabels,
}

impl DebugAction {
    pub fn input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();
        input_map.insert(
            Self::ShowTileLabels,
            ModifierKey::Control.with(KeyCode::KeyI),
        );
        input_map
    }
}
