use std::sync::LazyLock;

use bevy::prelude::*;
use derive_more::Display;
use serde::{Deserialize, Serialize};

use crate::input_dialog::{InputDialogCallback, InputDialogValue};
use crate::peer::start_matchbox_socket;
use crate::state::{InputDialogState, MultiplayerState};
use crate::unit::ActionsLegend;

const GAME_SESSION_ID_WORD_LEN: usize = 2;

static BIP39_ENGLISH_WORDLIST: LazyLock<Vec<String>> = LazyLock::new(|| {
    let wordlist = include_str!(concat!(env!("BEVY_ASSET_ROOT"), "/assets/bip39-english.txt")).trim_end();
    wordlist.split('\n').map(|word| word.to_owned()).collect()
});

#[derive(Clone, Hash, Debug, Deserialize, Serialize)]
pub struct GameSetup {
    pub map_seed: u64,
    pub game_seed: u64,
    pub num_players: u8,
}

#[derive(Debug, Display, Resource)]
#[display("{}", _0.join("-"))]
pub struct GameSessionId(pub [String; GAME_SESSION_ID_WORD_LEN]);

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
    actions_legend_text_query: Single<(&mut Text,), With<ActionsLegend>>,
) {
    let (mut actions_legend_text,) = actions_legend_text_query.into_inner();

    let game_session_id = GameSessionId(
        fastrand::choose_multiple(&*BIP39_ENGLISH_WORDLIST, GAME_SESSION_ID_WORD_LEN)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
    );
    let num_players = NumPlayers(2);

    actions_legend_text.0 = format!(
        "Hosting game...\nGame session ID: {game_session_id}\nPlayers: {num_players}\n",
        num_players = num_players.0
    );

    commands.insert_resource(game_session_id);
    commands.insert_resource(num_players);
    commands.insert_resource(MapRng(fastrand::Rng::new()));
    commands.insert_resource(GameRng(fastrand::Rng::new()));

    commands.run_system_cached(start_matchbox_socket);

    next_multiplayer_state.set(MultiplayerState::Hosting);
}

pub fn join_game(
    mut commands: Commands,
    mut next_multiplayer_state: ResMut<NextState<MultiplayerState>>,
    mut next_input_dialog_state: ResMut<NextState<InputDialogState>>,
    actions_legend_text_query: Single<(&mut Text,), With<ActionsLegend>>,
) {
    let (mut actions_legend_text,) = actions_legend_text_query.into_inner();

    actions_legend_text.0 = "Joining game...\n".to_owned();

    next_multiplayer_state.set(MultiplayerState::Joining);

    let callback_system_id = commands.register_system(join_game_callback);
    commands.insert_resource(InputDialogCallback(callback_system_id));
    next_input_dialog_state.set(InputDialogState::Shown);
}

fn join_game_callback(
    mut commands: Commands,
    input_dialog_value: Res<InputDialogValue>,
    mut next_input_dialog_state: ResMut<NextState<InputDialogState>>,
) {
    let words: Result<[String; 2], _> = input_dialog_value
        .0
        .split(&['-', ' '])
        .filter_map(|word| {
            if BIP39_ENGLISH_WORDLIST.iter().any(|w| w == word) {
                Some(word.to_owned())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .try_into();
    let Ok(words) = words else {
        // Invalid game session ID - let the user try to input again.
        // TODO: Some kind of error indication?
        return;
    };
    commands.insert_resource(GameSessionId(words));
    commands.insert_resource(NumPlayers(2));

    commands.run_system_cached(start_matchbox_socket);

    commands.remove_resource::<InputDialogCallback>();
    next_input_dialog_state.set(InputDialogState::Hidden);
}
