use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Hash, Debug, Deserialize, Serialize)]
pub struct GameSetup {
    pub map_seed: u64,
    pub game_seed: u64,
    pub num_players: u8,
}

#[derive(Resource)]
pub struct MapRng(pub fastrand::Rng);

#[derive(Resource)]
pub struct GameRng(pub fastrand::Rng);
