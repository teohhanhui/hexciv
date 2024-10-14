use bevy::ecs::event::Event;
use bevy_matchbox::prelude::PeerId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Event, Serialize)]
pub struct PlayerConnectedEvent {
    pub player_slot_index: u8,
    pub peer_id: PeerId,
}
