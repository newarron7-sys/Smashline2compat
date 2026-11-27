//! GGST clips in the existing slot-local sound bank. Timings follow BBS_SLY.
use smash::{app::sv_animcmd::frame, lua2cpp::L2CAgentBase, phx::Hash40};
use smash_script::macros;
use smashline::{Agent, Priority};

unsafe fn cue(a: &mut L2CAgentBase, at: f32, tone: &str) {
    frame(a.lua_state_agent, at);
    if macros::is_excute(a) {
        macros::PLAY_SE(a, Hash40::new(tone));
    }
}

pub unsafe extern "C" fn mappa_p(a: &mut L2CAgentBase) {
    cue(a, 10.0, "vc_kamui_special_n01");
}
pub unsafe extern "C" fn mappa_k(a: &mut L2CAgentBase) {
    // Tap/hold is committed at frame 5 in Smash.
    cue(a, 6.0, "se_kamui_special_n01");
    cue(a, 6.0, "se_kamui_special_n02");
    cue(a, 13.0, "vc_kamui_special_n02");
}
pub unsafe extern "C" fn pile(a: &mut L2CAgentBase) {
    cue(a, 4.0, "vc_kamui_special_s01");
    cue(a, 4.0, "se_kamui_special_n01");
    cue(a, 4.0, "se_kamui_special_n02");
    cue(a, 4.0, "se_kamui_special_s01");
}
pub unsafe extern "C" fn bump(a: &mut L2CAgentBase) {
    cue(a, 12.0, "vc_kamui_special_s02");
}
pub unsafe extern "C" fn late(a: &mut L2CAgentBase) {
    cue(a, 20.0, "vc_kamui_attack06");
    cue(a, 20.0, "se_kamui_special_n01");
}
pub unsafe extern "C" fn hammer(a: &mut L2CAgentBase) {
    cue(a, 2.0, "vc_kamui_attack07");
}
unsafe extern "C" fn low_slash(a: &mut L2CAgentBase) {
    // Source NmlAtk2C: SE_BTL_SLY_07 on cell sly232_01 (frame 4); cues here run one frame early like the poses.
    cue(a, 3.0, "se_kamui_attackhard_l01");
}
unsafe extern "C" fn wild_assault(a: &mut L2CAgentBase) {
    cue(a, 3.0, "vc_kamui_attack05");
}

unsafe extern "C" fn back_air(a: &mut L2CAgentBase) {
    // Source NmlAtkAir5D: SE_BTL_SLY_07 on cell sly253_02 (frame 8).
    cue(a, 7.0, "se_kamui_attackair_b01");
}
unsafe extern "C" fn heavy_tilt(a: &mut L2CAgentBase) {
    cue(a, 4.0, "se_kamui_special_n01");
    cue(a, 16.0, "se_kamui_special_s02");
}
pub unsafe extern "C" fn dandy_forward(a: &mut L2CAgentBase) {
    cue(a, 4.0, "se_kamui_special_n01");
    cue(a, 4.0, "se_kamui_special_s02");
}
pub unsafe extern "C" fn dandy_backward(a: &mut L2CAgentBase) {
    cue(a, 0.0, "se_kamui_special_n01");
    cue(a, 17.0, "se_kamui_special_n01");
    cue(a, 17.0, "se_kamui_special_s02");
}

pub unsafe extern "C" fn bloodsucking_universe(a: &mut L2CAgentBase) {
    cue(a, 4.0, "vc_kamui_appeal01");
    cue(a, 12.0, "se_kamui_appeal_l01");
    cue(a, 48.0, "se_kamui_appeal_l02");
}

pub unsafe extern "C" fn air_bite_attempt(a: &mut L2CAgentBase) {
    cue(a, 8.0, "vc_kamui_dragon");
}

pub unsafe extern "C" fn air_bite_release(a: &mut L2CAgentBase) {
    cue(a, 4.0, "vc_kamui_final_dragon");
    cue(a, 35.0, "se_kamui_special_l03");
}

pub unsafe extern "C" fn super_mappa(a: &mut L2CAgentBase) {
    // Source sly_ULT_02: vxxx256 with SE_BTL_SYS_SKill on frame 14.
    cue(a, 13.0, "vc_kamui_appeal02");
}
pub fn install() {
    Agent::new("captain").set_costume(crate::slots())
        .sound_acmd("sound_attacks3lw", low_slash, Priority::High)
        .sound_acmd("sound_attackairb", back_air, Priority::High)
        .sound_acmd("sound_slayer6htilt", heavy_tilt, Priority::High)
        .sound_acmd("sound_attackdash", wild_assault, Priority::High)
        .install();
}
