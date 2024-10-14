use bevy::color::Srgba;
use bevy::ecs::component::Component;
use bevy::ecs::system::Resource;
use derive_more::Display;
use strum::VariantArray;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display, Component, VariantArray)]
pub enum Civilization {
    America,
    Arabia,
    Brazil,
    China,
    Egypt,
    France,
    Germany,
    Greece,
    India,
    Japan,
    Kongo,
    Norway,
    Portugal,
    Rome,
    Russia,
    Scythia,
    Spain,
    Sumeria,
}

#[derive(Resource)]
pub struct OurCivilization(pub Civilization);

impl Civilization {
    pub fn colors(&self) -> [Srgba; 2] {
        match self {
            Civilization::America => [Srgba::hex("#042C6C"), Srgba::hex("#F7F7F8")],
            Civilization::Arabia => [Srgba::hex("#F3DB04"), Srgba::hex("#166C33")],
            Civilization::Brazil => [Srgba::hex("#64BC24"), Srgba::hex("#F2DB04")],
            Civilization::China => [Srgba::hex("#146C34"), Srgba::hex("#F9F9F9")],
            Civilization::Egypt => [Srgba::hex("#044C54"), Srgba::hex("#E8E19A")],
            Civilization::France => [Srgba::hex("#044CCC"), Srgba::hex("#E9E19D")],
            Civilization::Germany => [Srgba::hex("#ABABAB"), Srgba::hex("#1D1D1D")],
            Civilization::Greece => [Srgba::hex("#74A4F4"), Srgba::hex("#FAFAFB")],
            Civilization::India => [Srgba::hex("#340464"), Srgba::hex("#04C29B")],
            Civilization::Japan => [Srgba::hex("#FBFBFB"), Srgba::hex("#7C0505")],
            Civilization::Kongo => [Srgba::hex("#F4DB04"), Srgba::hex("#CC1514")],
            Civilization::Norway => [Srgba::hex("#042C6C"), Srgba::hex("#C91416")],
            Civilization::Portugal => [Srgba::hex("#FBFBFB"), Srgba::hex("#062D6C")],
            Civilization::Rome => [Srgba::hex("#6C04CB"), Srgba::hex("#F3DB07")],
            Civilization::Russia => [Srgba::hex("#F3DB04"), Srgba::hex("#1D1C1B")],
            Civilization::Scythia => [Srgba::hex("#FBB33B"), Srgba::hex("#7C0604")],
            Civilization::Spain => [Srgba::hex("#CC1414"), Srgba::hex("#F3DB04")],
            Civilization::Sumeria => [Srgba::hex("#042C6C"), Srgba::hex("#FA8316")],
        }
        .map(|color| color.expect("civilization hex colors should be valid"))
    }
}
