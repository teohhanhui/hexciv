use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Actionlike, Reflect)]
pub enum CursorAction {
    Click,
    SecondaryClick,
}

impl CursorAction {
    pub fn input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();
        input_map.insert(Self::Click, MouseButton::Left);
        input_map.insert(Self::SecondaryClick, MouseButton::Right);
        input_map
    }
}
