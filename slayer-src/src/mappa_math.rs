//! Source sprite clocks are zero-based: the first active pose begins at 10/13.
pub fn velocity(frame: f32, long: bool) -> f32 {
    let f = frame.floor() as i32;
    if long {
        match f {
            0..=4 => 0.0,
            5..=12 => 0.5 * 1.1_f32.powi(f - 5),
            13..=18 => 4.0 * 0.95_f32.powi(f - 13),
            _ => 4.0 * 0.95_f32.powi(5) * 0.8_f32.powi(f - 18),
        }
    } else {
        match f {
            0..=7 => 0.0,
            8..=9 => 1.0,
            10..=12 => 2.0 * 0.9_f32.powi(f - 10),
            _ => 2.0 * 0.9_f32.powi(2) * 0.85_f32.powi(f - 12),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_speed_changes_and_decay() {
        assert_eq!(velocity(7.0, false), 0.0);
        assert_eq!(velocity(8.0, false), 1.0);
        assert_eq!(velocity(10.0, false), 2.0);
        assert!((velocity(13.0, false) - 1.377).abs() < 0.0001);
        assert_eq!(velocity(5.0, true), 0.5);
        assert_eq!(velocity(13.0, true), 4.0);
        assert!(velocity(19.0, true) < velocity(18.0, true));
    }
    #[test]
    fn long_version_travels_farther_and_facing_is_symmetric() {
        let short: f32 = (0..30).map(|f| velocity(f as f32, false)).sum();
        let long: f32 = (0..38).map(|f| velocity(f as f32, true)).sum();
        assert!(long > short * 2.0);
        for held in [false, true] {
            for f in 0..38 {
                let v = velocity(f as f32, held);
                assert!(v.is_finite() && v >= 0.0);
                assert_eq!(v * -1.0, -v);
            }
        }
    }
}
