use std::cmp::Ordering;

use bevy::prelude::*;
use strum::VariantArray as _;

use crate::civilization::Civilization;
use crate::game_setup::{GameRng, NumPlayers};
use crate::peer::{OurPeerId, Peer};

#[derive(Debug, Resource)]
pub struct OurPlayer(pub Entity);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Component)]
#[require(PlayerState)]
pub struct Player {
    pub player_index: PlayerIndex,
    pub civ: Civilization,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct PlayerIndex(pub u8);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default, Component)]
pub enum PlayerState {
    #[default]
    Playing,
    WaitingForTurnEnd,
}

impl Ord for Player {
    fn cmp(&self, other: &Self) -> Ordering {
        self.player_index.cmp(&other.player_index)
    }
}

impl PartialOrd for Player {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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

    commands.spawn_batch(civs.into_iter().enumerate().map(|(i, &civ)| Player {
        player_index: PlayerIndex(u8::try_from(i).unwrap()),
        civ,
    }));
}

pub fn init_our_player(
    mut commands: Commands,
    our_peer_id: Res<OurPeerId>,
    peer_query: Query<(&Peer,), With<Peer>>,
    player_query: Query<(Entity, &Player), With<Player>>,
) {
    let Some((our_peer,)) = peer_query
        .iter()
        .find(|&(&peer,)| peer.peer_id == our_peer_id.0)
    else {
        commands.remove_resource::<OurPlayer>();
        return;
    };
    let (player_entity, _player) = player_query
        .iter()
        .find(|&(_entity, player)| player.player_index == our_peer.player_index)
        .ok_or_else(|| {
            format!(
                "Could not find Player with {our_player_index:?}",
                our_player_index = our_peer.player_index
            )
        })
        .unwrap();

    commands.insert_resource(OurPlayer(player_entity));
}
