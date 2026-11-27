#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JumpDirection {
    Neutral,
    Forward,
    Backward,
}
pub fn jump_direction(input: f32, facing: f32) -> JumpDirection {
    if input.abs() <= 0.2 {
        JumpDirection::Neutral
    } else if input * facing > 0.0 {
        JumpDirection::Forward
    } else {
        JumpDirection::Backward
    }
}
pub fn backward_input(input: f32, facing: f32) -> bool {
    input * facing < -0.2
}
pub fn teleport_velocity(frame: f32, back: bool, facing: f32) -> f32 {
    let speed = if back {
        if frame < 6.0 {
            -1.0
        } else if frame < 19.0 {
            -0.7
        } else {
            0.0
        }
    } else if frame >= 6.0 && frame < 11.0 {
        3.0
    } else {
        0.0
    };
    speed * facing
}
#[derive(Clone, Copy)]
pub enum Lunge {
    Sweep,
    FarSlash,
    ForwardHeavy,
    WildAssault,
}

pub fn attack_speed(attack: Lunge, frame: i32) -> f32 {
    match attack {
        Lunge::Sweep => match frame {
            6..=8 => 0.5,
            9..=23 => 3.0 * 0.9_f32.powi(frame - 9),
            _ => 0.0,
        },
        Lunge::FarSlash => match frame {
            2..=6 => 1.0 + 2.0 * 0.8_f32.powi(frame - 2),
            7..=14 => 2.0 * 0.8_f32.powi(5) * 0.7_f32.powi(frame - 7),
            _ => 0.0,
        },
        Lunge::ForwardHeavy => match frame {
            4..=15 => (4.0 * 2.0 / 3.0) * 0.8_f32.powi(frame - 4),
            16..=19 | 34..=36 => 0.25 * 2.0 / 3.0,
            _ => 0.0,
        },
        Lunge::WildAssault => match frame {
            7..=17 => 4.8 * 0.8_f32.powi(frame - 7),
            _ => 0.0,
        },
    }
}
/// Per-frame speed over the five hidden frames (6-10): 10 x 5 = 50 units (was 6.3, 31.5 units).
pub const RECOVERY_SPEED: f32 = 10.0;
/// Aim within the upper half-plane. A neutral stick goes straight up.
pub fn recovery_velocity(stick_x: f32, stick_y: f32) -> (f32, f32) {
    if stick_x.hypot(stick_y) < 0.3 {
        return (0.0, RECOVERY_SPEED);
    }
    let x = stick_x.clamp(-1.0, 1.0);
    let y = stick_y.clamp(0.33, 1.0);
    let scale = RECOVERY_SPEED / x.hypot(y);
    (x * scale, y * scale)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forward_and_back_are_relative_to_facing_not_screen_side() {
        for facing in [-1.0, 1.0] {
            assert!(!backward_input(facing, facing));
            assert!(backward_input(-facing, facing));
            assert_eq!(teleport_velocity(7.0, false, facing), 3.0 * facing);
            assert_eq!(teleport_velocity(7.0, true, facing), -0.7 * facing);
        }
    }
    #[test]
    fn teleport_direction_is_a_world_space_mirror() {
        for back in [false, true] {
            for frame in 0..28 {
                assert_eq!(
                    teleport_velocity(frame as f32, back, -1.0),
                    -teleport_velocity(frame as f32, back, 1.0)
                );
            }
        }
    }
    #[test]
    fn teleport_windows_stop_before_reorienting() {
        assert_eq!(teleport_velocity(5.0, false, 1.0), 0.0);
        assert_eq!(teleport_velocity(6.0, false, 1.0), 3.0);
        assert_eq!(teleport_velocity(11.0, false, 1.0), 0.0);
        assert_eq!(teleport_velocity(19.0, true, 1.0), 0.0);
    }
    #[test]
    fn all_three_jump_choices_work_on_both_sides() {
        for facing in [-1.0, 1.0] {
            assert_eq!(jump_direction(0.0, facing), JumpDirection::Neutral);
            assert_eq!(jump_direction(0.2, facing), JumpDirection::Neutral);
            assert_eq!(jump_direction(facing, facing), JumpDirection::Forward);
            assert_eq!(jump_direction(-facing, facing), JumpDirection::Backward);
        }
    }
}
