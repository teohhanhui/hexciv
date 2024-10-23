use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Actionlike, Reflect)]
pub enum UnitAction {
    SkipTurn,
    Fortify,
}

impl UnitAction {
    pub fn input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();
        input_map.insert(Self::SkipTurn, KeyCode::Space);
        input_map.insert(Self::Fortify, KeyCode::KeyF);
        input_map
    }
}
