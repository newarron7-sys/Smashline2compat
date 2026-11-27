//! Convert a desired two-player hitstop duration to Smash's ACMD multiplier.
pub(crate) const fn multiplier(damage: f32, frames: f32) -> f32 {
    // A small margin keeps roundoff from flooring an integral target one frame early.
    (frames + 0.125) / (damage / 2.6 + 5.0)
}
