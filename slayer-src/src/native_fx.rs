//! v0.3.80: GGST sprite layers that live in Slayer's native slot eff (no actor object cap). Each set is spawned at
//! the source frame of the BBScript object that links its particle system (v037/source-effects.json).
//! Static objects (no LinkObjectPosition) use EFFECT; objects linked to the player use EFFECT_FOLLOW.
use smash::{app::lua_bind::*, app::sv_animcmd::frame, lib::lua_const::*, lua2cpp::L2CAgentBase, phx::Hash40};
use smash_script::macros;

/// Waits for `at` (call in frame order inside a script) and spawns the set.
pub unsafe fn at(a: &mut L2CAgentBase, at: f32, set: &str, follow: bool) {
    frame(a.lua_state_agent, at);
    now(a, set, follow);
}

pub unsafe fn now(a: &mut L2CAgentBase, set: &str, follow: bool) {
    spawn(a, set, follow, 0.0);
}

/// Static spawn `fwd` units ahead of "top" (EFFECT offsets are in top's facing frame: z = forward).
pub unsafe fn ahead(a: &mut L2CAgentBase, at: f32, set: &str, fwd: f32) {
    frame(a.lua_state_agent, at);
    spawn(a, set, false, fwd);
}

/// v0.3.85: Slayer faces left through a negative X scale on "top" (locomotion.rs mirror). That joint matrix is a
/// reflection, which has no rotation the effect system can read, so left-facing sets kept their right-facing
/// orientation. Turn them ourselves (top's +Z forward onto world -X) and place forward offsets in world space.
unsafe fn spawn(a: &mut L2CAgentBase, set: &str, follow: bool, fwd: f32) {
    if !macros::is_excute(a) {
        return;
    }
    let b = a.module_accessor;
    let name = crate::slot_effect(b, set);
    let inverted = super::locomotion::mirror_inverted(b);
    let ry = if inverted { -90.0 } else { 0.0 };
    if follow {
        macros::EFFECT_FOLLOW(a, name, Hash40::new("top"), 0.0, 0.0, fwd, 0.0, ry, 0.0, 1.0, true);
    } else {
        macros::EFFECT(a, name, Hash40::new("top"), 0.0, 0.0, fwd, 0.0, ry, 0.0, 1.0, 0, 0, 0, 0, 0, 0, true);
    }
    if inverted && fwd != 0.0 && !follow {
        let handle = EffectModule::get_last_handle(b) as u32;
        let p = *PostureModule::pos(b);
        let scale = PostureModule::scale(b);
        EffectModule::set_pos(b, handle, &smash::phx::Vector3f { x: p.x - fwd * scale, y: p.y, z: p.z });
    }
}

/// SLY_dashstep_cancel: the source creates it on frame 1 only when the special was entered from a forward dash.
pub unsafe fn dashstep(a: &mut L2CAgentBase) {
    frame(a.lua_state_agent, 1.0);
    let prev = StatusModule::prev_status_kind(a.module_accessor, 0);
    if prev == *FIGHTER_STATUS_KIND_DASH || prev == *FIGHTER_STATUS_KIND_RUN || prev == *FIGHTER_STATUS_KIND_TURN_DASH {
        now(a, "slayer_dashstep_cancel", false);
    }
}
