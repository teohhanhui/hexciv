use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::state::MultiplayerState;
use crate::unit::ActionsLegend;

#[derive(Clone, Hash, Debug, Deserialize, Serialize)]
pub struct GameSetup {
    pub map_seed: u64,
    pub game_seed: u64,
    pub num_players: u8,
}

#[derive(Debug, Resource)]
pub struct MapRng(pub fastrand::Rng);

#[derive(Debug, Resource)]
pub struct GameRng(pub fastrand::Rng);

#[derive(Debug, Resource)]
pub struct NumPlayers(pub u8);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct GameSetupSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct InGameSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct HostingSet;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, SystemSet)]
pub struct JoiningSet;

pub fn host_game(
    mut commands: Commands,
    mut next_multiplayer_state: ResMut<NextState<MultiplayerState>>,
    mut actions_legend_text_query: Query<(&mut Text,), With<ActionsLegend>>,
) {
    let (mut actions_legend_text,) = actions_legend_text_query.get_single_mut().unwrap();

    actions_legend_text.sections[0].value = "Hosting game...\n".to_owned();

    commands.insert_resource(MapRng(fastrand::Rng::new()));
    commands.insert_resource(GameRng(fastrand::Rng::new()));
    commands.insert_resource(NumPlayers(2));

    next_multiplayer_state.set(MultiplayerState::Hosting);
}

pub fn join_game(
    mut next_multiplayer_state: ResMut<NextState<MultiplayerState>>,
    mut actions_legend_text_query: Query<(&mut Text,), With<ActionsLegend>>,
) {
    let (mut actions_legend_text,) = actions_legend_text_query.get_single_mut().unwrap();

    actions_legend_text.sections[0].value = "Joining game...\n".to_owned();

    next_multiplayer_state.set(MultiplayerState::Joining);
}
