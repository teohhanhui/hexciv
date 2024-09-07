// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#[derive(Debug)]
pub struct Config {
    /// Seed of the noise generator.
    pub seed: u32,

    /// Number of particles.
    /// The larger the value, the more the quality of the terrain is improved.
    pub particle_num: usize,

    /// (advanced) Power of the erodibility distribution.
    /// The larger the value, the more the erodibility is concentrated on the
    /// lower side.
    pub erodibility_distribution_power: f64,

    /// (advanced) Scale of the fault.
    /// The larger the value, the more virtual faults effect the terrain.
    pub fault_scale: f64,

    /// (advanced) Approximate ratio of the land area (0.0-1.0).
    pub land_ratio: f64,

    /// (advanced) If true, the edge points of the terrain are always outlet and
    /// its elevation is fixed to 0.
    pub convex_hull_is_always_outlet: bool,

    /// (advanced) Maximum slope angle of the terrain.
    /// The larger the value, the more the terrain is rough (radian, max: Pi/2).
    pub global_max_slope: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: 0,
            particle_num: 50_000,
            erodibility_distribution_power: 4.0,
            fault_scale: 35.0,
            land_ratio: 0.6,
            convex_hull_is_always_outlet: false,
            global_max_slope: 1.57,
        }
    }
}
