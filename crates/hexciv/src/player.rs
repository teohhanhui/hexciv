use bevy::prelude::*;
use strum::VariantArray as _;

use crate::civilization::Civilization;
use crate::game_setup::{GameRng, NumPlayers};
use crate::peer::{OurPeerId, Peer, PeerId};

#[derive(Debug, Resource)]
pub struct OurPlayer(pub Entity);

#[derive(Component)]
pub struct Player;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Component)]
pub struct PlayerIndex(pub u8);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default, Component)]
pub enum PlayerState {
    #[default]
    Playing,
    WaitingForTurnEnd,
}

#[derive(Bundle)]
pub struct PlayerBundle {
    player: Player,
    pub player_index: PlayerIndex,
    pub civ: Civilization,
    pub player_state: PlayerState,
}

impl PlayerBundle {
    pub fn new(player_index: PlayerIndex, civ: Civilization) -> Self {
        Self {
            player: Player,
            player_index,
            civ,
            player_state: PlayerState::default(),
        }
    }
}

pub fn spawn_players(
    mut commands: Commands,
    mut game_rng: ResMut<GameRng>,
    num_players: Res<NumPlayers>,
) {
    let rng = &mut game_rng.0;
    info!(seed = rng.get_seed(), "game seed");

    let civs = rng.choose_multiple(Civilization::VARIANTS.iter(), num_players.0.into());

    commands.spawn_batch(
        civs.into_iter()
            .enumerate()
            .map(|(i, &civ)| PlayerBundle::new(PlayerIndex(u8::try_from(i).unwrap()), civ)),
    );
}

pub fn init_our_player(
    mut commands: Commands,
    our_peer_id: Res<OurPeerId>,
    peer_query: Query<(&PeerId, &PlayerIndex), With<Peer>>,
    player_query: Query<(Entity, &PlayerIndex), With<Player>>,
) {
    let (_our_peer_id, our_player_index) = peer_query
        .iter()
        .find(|(&peer_id, _player_index)| peer_id.0 == our_peer_id.0)
        .expect("our peer info should have been populated");
    let (player_entity, _player_index) = player_query
        .iter()
        .sort::<&PlayerIndex>()
        .nth(our_player_index.0.into())
        .unwrap();

    commands.insert_resource(OurPlayer(player_entity));
}
