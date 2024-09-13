// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

pub use fastlem::models::surface::sites::Site2D;
pub use fastlem::models::surface::terrain::Terrain2D;

pub use self::config::Config;
pub use self::generate::generate_terrain;

mod config;
mod generate;
mod math;
