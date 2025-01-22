use super::{trades::Trade, measurements::Measurement};

#[derive(Debug)]
pub struct Penalty {
    pub area_uuid: String,
    pub penalty_energy: f32,
}

pub fn compute_penalties(
    trades: &[Trade], 
    measurements: &[Measurement]
) -> Vec<Penalty> {
    // TODO: 
    // e.g. penalty = measured_energy - traded_energy if it's > 0, etc.
    // ...
    Vec::new() // placeholder
}
