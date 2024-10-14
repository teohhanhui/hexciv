use bevy::ecs::system::Resource;
use serde::{Deserialize, Serialize};

#[derive(Clone, Hash, Debug, Deserialize, Resource, Serialize)]
pub struct GameSetup {
    pub map_seed: u64,
    pub game_seed: u64,
    pub num_players: u8,
}
