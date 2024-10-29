use bevy::prelude::*;

#[derive(Deref, Resource)]
pub struct FontHandle(Handle<Font>);

impl FromWorld for FontHandle {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self(asset_server.load("fonts/NotoSansMono/NotoSansMono-Regular.ttf"))
    }
}
