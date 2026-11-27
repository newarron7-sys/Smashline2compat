//! GGST camera shakes (ShakeCameraEx / cmn_screenshake) at their source frames, mapped to Smash's fixed quake sizes:
//! up to 400 or cmn_screenshake level 1-2 -> S, 600-800 or level 3 -> M, 2000 and above -> L.
use smash::{app::sv_animcmd::frame, lib::lua_const::*, lua2cpp::L2CAgentBase};
use smash_script::macros;

#[derive(Clone, Copy)]
pub enum Size { S, M, L }

/// Waits for `at` (only call it in frame order inside a script) and shakes the camera.
pub unsafe fn at(a: &mut L2CAgentBase, at: f32, size: Size) {
    frame(a.lua_state_agent, at);
    now(a, size);
}

pub unsafe fn now(a: &mut L2CAgentBase, size: Size) {
    if macros::is_excute(a) {
        let kind = match size { Size::S => *CAMERA_QUAKE_KIND_S, Size::M => *CAMERA_QUAKE_KIND_M, Size::L => *CAMERA_QUAKE_KIND_L };
        macros::QUAKE(a, kind);
    }
}
