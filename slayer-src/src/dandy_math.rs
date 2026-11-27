//! Dandy Step source clocks are zero-based. Inputs are buffered for six frames.
pub fn window(back: bool, frame: f32) -> bool {
    frame >= 17.0 && frame < if back { 31.0 } else { 25.0 }
}
pub fn velocity(back: bool, frame: f32) -> f32 {
    let f = frame.floor() as i32;
    if back {
        match f {
            0..=1 => 0.0,
            2..=16 => -3.5 * 0.7_f32.powi(f - 2),
            17..=20 => 2.25 * 1.05_f32.powi(f - 17),
            _ => 3.0 * 0.85_f32.powi(f - 21),
        }
    } else {
        match f {
            0..=3 => 0.0,
            4..=8 => 3.0 * 0.9_f32.powi(f - 4),
            _ => 3.0 * 0.9_f32.powi(4) * 0.8_f32.powi(f - 8),
        }
    }
}
pub fn followup(x: f32, y: f32, lr: f32) -> u8 {
    if y < -0.4 {
        2
    } else if y > 0.4 {
        4
    } else if x * lr > 0.4 {
        3
    } else {
        1
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_followup_windows_are_half_open() {
        for back in [false, true] {
            assert!(!window(back, 16.0));
            assert!(window(back, 17.0));
            let end = if back { 31.0 } else { 25.0 };
            assert!(window(back, end - 1.0));
            assert!(!window(back, end));
        }
    }
    #[test]
    fn retreat_then_advance() {
        assert_eq!(velocity(true, 0.0), 0.0);
        assert_eq!(velocity(true, 2.0), -3.5);
        assert!(velocity(true, 16.0) < 0.0);
        assert_eq!(velocity(true, 17.0), 2.25);
        assert!(velocity(true, 20.0) > velocity(true, 17.0));
        assert_eq!(velocity(true, 21.0), 3.0);
    }
    #[test]
    fn controls_follow_facing_and_vertical_priority() {
        for lr in [-1.0, 1.0] {
            assert_eq!(followup(0.0, 0.0, lr), 1);
            assert_eq!(followup(lr, 0.0, lr), 3);
            assert_eq!(followup(-lr, 0.0, lr), 1);
            assert_eq!(followup(lr, -1.0, lr), 2);
            assert_eq!(followup(lr, 1.0, lr), 4);
        }
    }
}
