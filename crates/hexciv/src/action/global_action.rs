use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Actionlike, Reflect)]
pub enum GlobalAction {
    PreviousReadyUnit,
    NextReadyUnit,
}

impl GlobalAction {
    pub fn input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();
        input_map.insert(Self::PreviousReadyUnit, KeyCode::Comma);
        input_map.insert(Self::NextReadyUnit, KeyCode::Period);
        input_map.insert(Self::NextReadyUnit, KeyCode::KeyZ);
        input_map
    }
}
