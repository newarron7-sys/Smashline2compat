//! Source normal assignment and gatling routes (v0.3.18), kept free of game calls so it can be unit tested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Normal {
    P5,
    K5,
    CS,
    FS,
    P6,
    H6,
    K2,
    S2,
    H2,
    D2,
    JP,
    JK,
    JS,
    JH,
    JD,
}
use Normal::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cancel {
    Normal(Normal),
    SpecialN,
    SpecialS,
    Jump,
    Dash,
}
/// AddChain lists from the source actions (Dustloop's gatling tables agree), restricted to normals this mod
/// assigns. 2P, 6K, 5H and 5D are not on any Smash input yet, so their routes are left out rather than remapped.
/// Dustloop also lists j.K/j.S/j.H -> j.D, which the source AddChain lists omit (generic Strive rule).
pub fn gatlings(from: Normal) -> &'static [Normal] {
    match from {
        P5 => &[P5, P6, H6],
        K5 => &[P6, H6, D2],
        CS => &[P6, FS, S2, H2, H6, D2],
        FS => &[H2],
        K2 => &[P6, H6, D2],
        S2 => &[H2],
        JP => &[JP],
        // Dustloop air gatlings: j.K -> j.D, j.S -> j.H/j.D, j.H -> j.D.
        JK => &[JD],
        JS => &[JH, JD],
        JH => &[JD],
        _ => &[],
    }
}
pub fn grounded(n: Normal) -> bool {
    !matches!(n, JP | JK | JS | JH | JD)
}
/// 5K and c.S list Cancel_FDash in source and are jump cancellable (Dustloop). In the air, j.K, j.S, j.H
/// and j.D cancel into a jump or a forward air dash (Smash: double jump / air Side B).
pub fn jump_dash(n: Normal) -> bool {
    matches!(n, K5 | CS | JK | JS | JH | JD)
}
/// Dustloop's Cancel column: every grounded normal mapped here except 6H is special cancellable.
/// Slayer has no aerial specials in Strive, so air normals never cancel into Smash specials.
pub fn special(n: Normal) -> bool {
    grounded(n) && n != H6
}
pub fn allowed(from: Normal, to: Cancel) -> bool {
    match to {
        Cancel::Normal(n) => gatlings(from).contains(&n),
        Cancel::SpecialN | Cancel::SpecialS => special(from),
        Cancel::Jump | Cancel::Dash => jump_dash(from),
    }
}
/// Side tilt: back (relative to the opponent) is f.S, forward angled down is 2S, otherwise 5K.
pub fn side_tilt(stick_x: f32, stick_y: f32, facing: f32) -> Normal {
    if stick_y < -0.3 {
        down_tilt(stick_x, facing)
    } else if stick_x * facing < -0.2 {
        FS
    } else {
        K5
    }
}
pub fn up_tilt(stick_x: f32, facing: f32) -> Normal {
    if stick_x * facing < -0.3 { P6 } else { H6 }
}
/// Down tilt: down-forward is 2S, down-back is 2D, straight down is 2K.
pub fn down_tilt(stick_x: f32, facing: f32) -> Normal {
    let x = stick_x * facing;
    if x > 0.3 {
        S2
    } else if x < -0.3 {
        D2
    } else {
        K2
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jab_is_only_5p_and_chains_into_itself() {
        assert!(allowed(P5, Cancel::Normal(P5)));
        assert!(!allowed(P5, Cancel::Normal(K5)));
    }
    #[test]
    fn tilts_follow_the_opponent_not_the_screen() {
        for facing in [-1.0, 1.0] {
            assert_eq!(side_tilt(-0.8 * facing, 0.0, facing), FS);
            assert_eq!(side_tilt(0.8 * facing, 0.0, facing), K5);
            assert_eq!(side_tilt(0.7 * facing, -0.7, facing), S2);
            assert_eq!(side_tilt(-0.7 * facing, -0.7, facing), D2);
            assert_eq!(up_tilt(-0.7 * facing, facing), P6);
            assert_eq!(up_tilt(0.0, facing), H6);
            assert_eq!(down_tilt(0.7 * facing, facing), S2);
            assert_eq!(down_tilt(-0.7 * facing, facing), D2);
            assert_eq!(down_tilt(0.1 * facing, facing), K2);
        }
    }
    #[test]
    fn source_routes() {
        assert!(allowed(CS, Cancel::Normal(FS)));
        assert!(allowed(K5, Cancel::Normal(D2)));
        assert!(allowed(S2, Cancel::Normal(H2)));
        assert!(!allowed(H6, Cancel::Normal(P5)));
        assert!(allowed(JS, Cancel::Normal(JH)));
        assert!(!allowed(H6, Cancel::SpecialS) && !allowed(H6, Cancel::SpecialN));
        assert!(allowed(H2, Cancel::SpecialS) && allowed(D2, Cancel::SpecialN) && allowed(P6, Cancel::SpecialS));
        assert!(!allowed(JP, Cancel::SpecialN));
        assert!(allowed(K5, Cancel::Jump) && !allowed(P5, Cancel::Dash));
    }
    #[test]
    fn air_routes_match_dustloop() {
        for n in [JK, JS, JH] {
            assert!(allowed(n, Cancel::Normal(JD)));
        }
        assert!(allowed(JS, Cancel::Normal(JH)));
        assert!(!allowed(JD, Cancel::Normal(JD)) && !allowed(JP, Cancel::Normal(JD)));
        for n in [JK, JS, JH, JD] {
            assert!(allowed(n, Cancel::Jump) && allowed(n, Cancel::Dash));
            assert!(!allowed(n, Cancel::SpecialS));
        }
        assert!(!allowed(JP, Cancel::Jump) && !allowed(JP, Cancel::Dash));
    }
}
