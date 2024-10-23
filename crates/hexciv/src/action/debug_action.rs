use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

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
