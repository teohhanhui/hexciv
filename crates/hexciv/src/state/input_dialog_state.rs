use bevy::prelude::*;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default, States)]
pub enum InputDialogState {
    #[default]
    Hidden,
    Shown,
}
