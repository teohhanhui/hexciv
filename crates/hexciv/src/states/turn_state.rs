use bevy::state::state::States;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default, States)]
pub enum TurnState {
    Playing,
    #[default]
    Processing,
}
