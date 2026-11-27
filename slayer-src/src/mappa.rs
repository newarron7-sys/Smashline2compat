//! Mappa Hunch. GGST pose holds/attack windows, Smash-scaled damage.
//! Source sly_SP_01/02 is ground-only; the air version (v0.3.21, user request) keeps the same timeline and drift,
//! starts with zero vertical speed, hangs until the first active frame, then falls at a quarter of Captain's air
//! gravity (half his fall speed), once per airtime.
use super::locomotion::{entry, facing, release_drive, settings, speed};
use smash::{
    app::lua_bind::*,
    lib::{lua_const::*, L2CValue},
    lua2cpp::{L2CAgentBase, L2CFighterCommon},
    phx::Hash40,
};
use smash_script::{lua_args, macros, sv_kinetic_energy};
use smashline::{Agent, CheckAttack, End, Exec, Exit, Init, Main, Pre, Priority};
#[derive(Clone, Copy)]
struct State {
    lr: f32,
    released: bool,
    chosen: bool,
    long: bool,
    active: bool,
    air: bool,
    vy: f32,
    overdrive: bool,
}
static mut STATES: [State; 8] = [State {
    lr: 1.0,
    released: false,
    chosen: false,
    long: false,
    active: false,
    air: false,
    vy: 0.0,
    overdrive: false,
}; 8];
static mut AIR_USED: [bool; 8] = [false; 8];
/// Air Mappa descent relative to Captain's air_accel_y / air_speed_y_stable.
const AIR_GRAVITY_SCALE: f32 = 0.25;
const AIR_FALL_SCALE: f32 = 0.5;
#[derive(Clone, Copy)]
struct Command {
    object: u32,
    steps: u8,
    age: u8,
    pending: u8,
    last_status: i32,
}
impl Command {
    const NEW: Self = Self { object: u32::MAX, steps: 0, age: 0, pending: 0, last_status: -1 };
}
static mut COMMANDS: [Command; 8] = [Command::NEW; 8];

unsafe fn command_ready(status: i32) -> bool {
    [*FIGHTER_STATUS_KIND_WAIT, *FIGHTER_STATUS_KIND_WALK, *FIGHTER_STATUS_KIND_DASH,
     *FIGHTER_STATUS_KIND_TURN, *FIGHTER_STATUS_KIND_SQUAT, *FIGHTER_STATUS_KIND_SQUAT_WAIT,
     *FIGHTER_STATUS_KIND_SQUAT_F, *FIGHTER_STATUS_KIND_SQUAT_B, *FIGHTER_STATUS_KIND_SQUAT_RV, 0x201,
     // la flèche seule lance la provocation : on peut enchaîner B pendant celle-ci
     *FIGHTER_STATUS_KIND_APPEAL]
        .contains(&status)
}

unsafe fn read_command(f: &mut L2CFighterCommon, e: usize) {
    let b = f.module_accessor;
    let command = &mut COMMANDS[e];
    if command.object != (*b).battle_object_id {
        *command = Command { object: (*b).battle_object_id, ..Command::NEW };
    }
    let status = StatusModule::status_kind(b);
    let can_start = command_ready(status) || (command_ready(command.last_status) &&
        [*FIGHTER_STATUS_KIND_ATTACK, *FIGHTER_STATUS_KIND_ATTACK_S3,
         *FIGHTER_STATUS_KIND_ATTACK_LW3, *FIGHTER_STATUS_KIND_ATTACK_S4]
            .contains(&status) && MotionModule::frame(b) < 2.0)
        // B parti un peu avant la flèche : le Mappa normal vient de commencer, on le relance en Super Mappa
        || (status == *FIGHTER_STATUS_KIND_SPECIAL_N && MotionModule::frame(b) < 4.0);
    command.last_status = status;
    command.pending = command.pending.saturating_sub(1);
    if !can_start || StatusModule::situation_kind(b) != *SITUATION_KIND_GROUND
        || !smash::app::sv_information::is_ready_go()
    {
        command.steps = 0;
        return;
    }
    command.age = command.age.saturating_add(1);
    if command.age > 12 {
        command.steps = 0;
    }
    // Modif (demandée) : flèche du haut (croix) tenue + B = Super Mappa, sans Tension (comme Rock Howard).
    // Remplace le 236236 + A.
    // Correctif crash : changer le statut tout de suite depuis cette boucle relançait la boucle dans la même image
    // (Super Mappa -> boucle -> Super Mappa ...) jusqu'à dépasser la pile. On fait comme l'original : une demande
    // traitée à l'image suivante, une seule fois (rien si une demande est déjà en cours ou si le Super Mappa tourne).
    if ControlModule::check_button_on(b, *CONTROL_PAD_BUTTON_APPEAL_HI)
        && ControlModule::check_button_trigger(b, *CONTROL_PAD_BUTTON_SPECIAL)
        && command.pending == 0
        && MotionModule::motion_kind(b) != smash::hash40("slayer_super_mappa")
    {
        command.steps = 0;
        command.pending = 2;
        StatusModule::change_status_request_from_script(b, *FIGHTER_STATUS_KIND_SPECIAL_N, false);
    }
}
const CHECK_SPECIAL_N_UNIQ: i32 = 0x38;
static mut ORIGINAL_CHECK: [usize; 8] = [0; 8];
unsafe extern "C" fn can_start(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return false.into();
    };
    if StatusModule::situation_kind(b) != *SITUATION_KIND_GROUND && AIR_USED[e] {
        return false.into();
    }
    let original = ORIGINAL_CHECK[e];
    if original != 0 && original != can_start as *const () as usize {
        let callback: unsafe extern "C" fn(&mut L2CFighterCommon) -> L2CValue =
            std::mem::transmute(original);
        return callback(f);
    }
    true.into()
}
unsafe extern "C" fn install_eligibility(f: &mut L2CFighterCommon) {
    let Some(e) = entry(f.module_accessor) else {
        return;
    };
    if StatusModule::situation_kind(f.module_accessor) == *SITUATION_KIND_GROUND {
        AIR_USED[e] = false;
    }
    read_command(f, e);
    let current = f.global_table[CHECK_SPECIAL_N_UNIQ]
        .try_get_ptr::<()>()
        .map_or(0, |p| p as usize);
    if current != can_start as *const () as usize {
        ORIGINAL_CHECK[e] = current;
        f.global_table[CHECK_SPECIAL_N_UNIQ].assign(&L2CValue::Ptr(can_start as *const () as _));
    }
}

unsafe extern "C" fn pre(f: &mut L2CFighterCommon) -> L2CValue {
    settings(
        f,
        StatusModule::situation_kind(f.module_accessor) != *SITUATION_KIND_GROUND,
    );
    0.into()
}
unsafe extern "C" fn noop(_: &mut L2CFighterCommon) -> L2CValue {
    0.into()
}
// Falcon's native on-hit status callback owns the special zoom. Mappa's
// hitboxes and the shared collision hook already handle its hit behavior.
unsafe extern "C" fn check_attack(
    _: &mut L2CFighterCommon,
    _: &L2CValue,
    _: &L2CValue,
) -> L2CValue {
    0.into()
}
unsafe extern "C" fn main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return 0.into();
    };
    let air = StatusModule::situation_kind(b) != *SITUATION_KIND_GROUND;
    if air && AIR_USED[e] {
        // Only a forced entry gets here (can_start blocks it). Drop the buffered input first, or FALL re-enters
        // SPECIAL_N immediately and the transitions nest until the stack overflows.
        STATES[e].active = false;
        ControlModule::clear_command_one(b, 0, *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_N);
        f.change_status((*FIGHTER_STATUS_KIND_FALL).into(), false.into());
        return 1.into();
    }
    AIR_USED[e] |= air;
    let overdrive = !air && COMMANDS[e].pending > 0; // sans Tension (modif demandée)
    COMMANDS[e].pending = 0;
    let lr = facing(b);
    STATES[e] = State {
        lr,
        released: false,
        chosen: false,
        long: false,
        active: true,
        air,
        vy: 0.0,
        overdrive,
    };
    PostureModule::set_lr(b, lr);
    PostureModule::update_rot_y_lr(b);
    AttackModule::clear_all(b);
    MotionModule::change_motion(
        b,
        Hash40::new(if overdrive { "slayer_super_mappa" } else { "slayer_mappa_p" }),
        0.0,
        1.0,
        false,
        0.0,
        false,
        false,
    );
    speed(f, 0.0);
    if air {
        fall(f, 0.0);
    }
    f.sub_shift_status_main(L2CValue::Ptr(loop_main as *const () as _))
}
unsafe extern "C" fn loop_main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return 0.into();
    };
    let air = StatusModule::situation_kind(b) != *SITUATION_KIND_GROUND;
    if air != STATES[e].air {
        let next = if air { *FIGHTER_STATUS_KIND_FALL } else { *FIGHTER_STATUS_KIND_LANDING };
        f.change_status(next.into(), false.into());
        return 1.into();
    }
    let fr = MotionModule::frame(b);
    if STATES[e].overdrive {
        if !StopModule::is_stop(b) {
            let speed_x = if (13.0..16.0).contains(&fr) {
                -4.0 * 0.6_f32.powf(fr - 13.0)
            } else if (159.0..164.0).contains(&fr) {
                7.5 * 0.95_f32.powf(fr - 159.0)
            } else if (164.0..207.0).contains(&fr) {
                // Strive halves the remaining rush velocity on frame 164
                // and starts its 90% braking curve on frame 167.
                7.5 * 0.95_f32.powi(5) * 0.5 * 0.9_f32.powf((fr - 167.0).max(0.0))
            } else {
                0.0
            };
            speed(f, speed_x * STATES[e].lr);
        }
        if MotionModule::is_end(b) {
            f.change_status((*FIGHTER_STATUS_KIND_WAIT).into(), false.into());
            return 1.into();
        }
        return 0.into();
    }
    if !STATES[e].chosen {
        if !ControlModule::check_button_on(b, *CONTROL_PAD_BUTTON_SPECIAL) {
            STATES[e].released = true;
        }
        if fr >= 5.0 {
            STATES[e].chosen = true;
            STATES[e].long = !STATES[e].released;
            if STATES[e].long {
                // Both source timelines share the first five frames. No startup is added.
                MotionModule::change_motion(
                    b,
                    Hash40::new("slayer_mappa_k"),
                    fr,
                    1.0,
                    false,
                    0.0,
                    false,
                    false,
                );
            }

        }
    }
    if !StopModule::is_stop(b) {
        speed(
            f,
            super::mappa_math::velocity(fr, STATES[e].long) * STATES[e].lr,
        );
        if air {
            // speed() parks gravity each frame; integrate a reduced air gravity here instead (v0.3.34: the user
            // found Captain's full gravity too fast). Hang until the first active frame, then a slow fall.
            if fr >= if STATES[e].long { 14.0 } else { 11.0 } {
                let accel = WorkModule::get_param_float(b, smash::hash40("air_accel_y"), 0) * AIR_GRAVITY_SCALE;
                let stable = WorkModule::get_param_float(b, smash::hash40("air_speed_y_stable"), 0) * AIR_FALL_SCALE;
                STATES[e].vy = (STATES[e].vy - accel).max(-stable);
            }
            fall(f, STATES[e].vy);
        }
    }
    // N samples cover [0,N); let the final source pose receive its complete hold.
    if fr >= if STATES[e].long { 38.0 } else { 30.0 } {
        let next = if air { *FIGHTER_STATUS_KIND_FALL } else { *FIGHTER_STATUS_KIND_WAIT };
        f.change_status(next.into(), false.into());
        return 1.into();
    }
    0.into()
}
unsafe fn fall(f: &mut L2CFighterCommon, vy: f32) {
    sv_kinetic_energy!(reset_energy, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, ENERGY_GRAVITY_RESET_TYPE_GRAVITY, 0.0, vy, 0.0, 0.0, 0.0);
    sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, vy);
    sv_kinetic_energy!(set_accel, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 0.0);
    sv_kinetic_energy!(set_stable_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 10.0);
    sv_kinetic_energy!(set_limit_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 10.0);
    KineticModule::enable_energy(f.module_accessor, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
}
unsafe extern "C" fn end(f: &mut L2CFighterCommon) -> L2CValue {
    super_invuln(f.module_accessor, false);
    let Some(e) = entry(f.module_accessor) else {
        return 0.into();
    };
    if !STATES[e].active {
        return 0.into();
    }
    STATES[e].active = false;
    AttackModule::clear_all(f.module_accessor);
    release_drive(f.module_accessor);
    0.into()
}
unsafe fn hit(a: &mut L2CAgentBase, long: bool) {
    let damage = if long { 8.0 } else { 6.0 };
    // Capsule fitted inside the source JONB attack rectangle, scale .05.
    // Native Smash collision is round-ended; this is not a rectangular engine port.
    macros::ATTACK(
        a,
        0,
        0,
        Hash40::new("top"),
        damage,
        if long { 15 } else { 25 },
        75,
        0,
        if long { 45 } else { 30 },
        3.5,
        0.0,
        10.375,
        8.125,
        Some(0.0),
        Some(10.375),
        Some(20.0),
        super::hitstop::multiplier(damage, if long { 20.0 } else { 18.0 }),
        1.0,
        *ATTACK_SETOFF_KIND_OFF,
        *ATTACK_LR_CHECK_F,
        false,
        0,
        0.0,
        0,
        false,
        false,
        false,
        false,
        true,
        *COLLISION_SITUATION_MASK_GA,
        *COLLISION_CATEGORY_MASK_ALL,
        *COLLISION_PART_MASK_ALL,
        false,
        Hash40::new("collision_attr_normal"),
        *ATTACK_SOUND_LEVEL_L,
        *COLLISION_SOUND_ATTR_PUNCH,
        *ATTACK_REGION_PUNCH,
    );
    AttackModule::set_no_finish_camera(a.module_accessor, 0, true, false);
}
unsafe extern "C" fn p(a: &mut L2CAgentBase) {
    // Source sly_SP_01 / sly_SP_02 attack cells: P 11-13, K 14-19 (Dustloop 11/3, 14/6).
    super::native_fx::dashstep(a); // SLY_dashstep_cancel at 1 (from a dash)
    super::native_fx::at(a, 3.0, "slayer_400_punch_sub_glow", true); // SLY_400_punch_sub at 3
    super::shake::at(a, 9.0, super::shake::Size::S); // ShakeCameraEx Vertical 400 at 9
    smash::app::sv_animcmd::frame(a.lua_state_agent, 11.0);
    if macros::is_excute(a) {
        hit(a, false);
    }
    super::native_fx::now(a, "slayer_400_punch_glow", true); // SLY_400_punch at 11
    smash::app::sv_animcmd::frame(a.lua_state_agent, 14.0);
    if macros::is_excute(a) {
        AttackModule::clear_all(a.module_accessor);
    }
}
unsafe extern "C" fn k(a: &mut L2CAgentBase) {
    super::native_fx::dashstep(a); // SLY_dashstep_cancel at 1 (from a dash)
    super::native_fx::at(a, 6.0, "slayer_400_punch_sub_glow", true); // SLY_400_punch_sub at 6
    super::shake::at(a, 12.0, super::shake::Size::M); // ShakeCameraEx Vertical 600 at 12
    super::native_fx::now(a, "slayer_stone_burst", false); // SLY_400_stoneburst at 12
    smash::app::sv_animcmd::frame(a.lua_state_agent, 14.0);
    if macros::is_excute(a) {
        hit(a, true);
    }
    super::native_fx::now(a, "slayer_400_punch_k_glow", true); // SLY_400_punch_K at 14
    smash::app::sv_animcmd::frame(a.lua_state_agent, 20.0);
    if macros::is_excute(a) {
        AttackModule::clear_all(a.module_accessor);
    }
}
/// Set while Super Mappa's source strike/throw invulnerability is applied, so End can always undo it.
static mut SUPER_INVULN: [bool; 8] = [false; 8];
unsafe fn super_invuln(b: *mut smash::app::BattleObjectModuleAccessor, on: bool) {
    let Some(e) = entry(b) else { return };
    if on == SUPER_INVULN[e] {
        return;
    }
    SUPER_INVULN[e] = on;
    let status = if on { *HIT_STATUS_XLU } else { *HIT_STATUS_NORMAL };
    HitModule::set_whole(b, smash::app::HitStatus(status), 0);
}
unsafe extern "C" fn super_mappa(a: &mut L2CAgentBase) {
    // Source sly_ULT_02: MutekiDageki/MutekiNage on at 14, off at 165; attack cells 160-164.
    super::native_fx::dashstep(a); // SLY_dashstep_cancel at 1 (from a dash)
    smash::app::sv_animcmd::frame(a.lua_state_agent, 14.0);
    if macros::is_excute(a) {
        super_invuln(a.module_accessor, true);
    }
    super::native_fx::now(a, "slayer_502_gareki00", false); // SLY_502_gareki00 at 14
    super::native_fx::at(a, 20.0, "slayer_502_gareki01", false);
    super::native_fx::at(a, 62.0, "slayer_502_gareki02", false);
    super::native_fx::at(a, 107.0, "slayer_502_gareki03", false);
    super::native_fx::at(a, 157.0, "slayer_502_gareki_final", false);
    // SLY_502_MANAGER (created at 157): floorstone01..04 at idling 3/7/11/15, ExPointF 0/400000/800000/1200000 + AddPosX 200000
    // (20000 GGST units = 1 Smash unit, as sly_SP_09_exe AddPosX 30000 = 1.5). Interleaved with the frame order below.
    super::native_fx::ahead(a, 160.0, "slayer_502_floorstone01", 10.0);
    smash::app::sv_animcmd::frame(a.lua_state_agent, 160.0);
    if macros::is_excute(a) {
        // The source rush starts its hit cinematic here. Smash resolves it as a single launching strike.
        macros::ATTACK(a, 0, 0, Hash40::new("top"), 22.0, 42, 85, 0, 48, 5.5,
            0.0, 10.5, 5.0, Some(0.0), Some(10.5), Some(19.0), super::hitstop::multiplier(22.0, 26.0), 1.0,
            *ATTACK_SETOFF_KIND_OFF, *ATTACK_LR_CHECK_F, false, 0, 0.0, 0,
            false, false, false, false, true, *COLLISION_SITUATION_MASK_GA,
            *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false,
            Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_L,
            *COLLISION_SOUND_ATTR_PUNCH, *ATTACK_REGION_PUNCH);
        AttackModule::set_no_finish_camera(a.module_accessor, 0, true, false);
    }
    super::native_fx::ahead(a, 164.0, "slayer_502_floorstone02", 30.0);
    smash::app::sv_animcmd::frame(a.lua_state_agent, 165.0);
    if macros::is_excute(a) {
        AttackModule::clear_all(a.module_accessor);
        super_invuln(a.module_accessor, false);
    }
    super::native_fx::ahead(a, 168.0, "slayer_502_floorstone03", 50.0);
    super::native_fx::ahead(a, 172.0, "slayer_502_floorstone04", 70.0);
}
unsafe extern "C" fn empty(_: &mut L2CAgentBase) {}
pub fn install() {
    Agent::new("captain")
        .set_costume(crate::slots())
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_N, pre)
        .status(Init, *FIGHTER_STATUS_KIND_SPECIAL_N, noop)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_N, main)
        .status(Exec, *FIGHTER_STATUS_KIND_SPECIAL_N, noop)
        .status(CheckAttack, *FIGHTER_STATUS_KIND_SPECIAL_N, check_attack)
        .status(End, *FIGHTER_STATUS_KIND_SPECIAL_N, end)
        .status(Exit, *FIGHTER_STATUS_KIND_SPECIAL_N, noop)
        .game_acmd("game_slayersupermappa", super_mappa, Priority::High)
        .sound_acmd("sound_slayersupermappa", super::audio::super_mappa, Priority::High)
        .effect_acmd("effect_slayersupermappa", empty, Priority::High)
        .expression_acmd("expression_slayersupermappa", empty, Priority::High)
        .game_acmd("game_slayermappap", p, Priority::High)
        .game_acmd("game_slayermappak", k, Priority::High)
        .sound_acmd("sound_slayermappap", super::audio::mappa_p, Priority::High)
        .sound_acmd("sound_slayermappak", super::audio::mappa_k, Priority::High)
        .expression_acmd("expression_slayermappap", empty, Priority::High)
        .expression_acmd("expression_slayermappak", empty, Priority::High)
        .on_start(install_eligibility)
        .on_line(Main, install_eligibility)
        .install();
}
