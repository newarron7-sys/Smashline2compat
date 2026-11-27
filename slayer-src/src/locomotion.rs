//! Source conventions from the owned BBS_SLY: Source distances are scaled to the imported character size (20,000 script units per model unit).
use smash::{
    app::{self, GroundCliffCheckKind, SituationKind, lua_bind::*},
    lib::{L2CValue, lua_const::*},
    lua2cpp::L2CFighterCommon,
    phx::{Hash40, Vector3f},
};
use smash_script::{lua_args, sv_kinetic_energy};
use smashline::{Agent, End, Exec, Exit, Init, Main, Pre, Priority};
#[derive(Clone, Copy)]
struct State {
    object: u32,
    air_used: bool,
    air_active: bool,
    recovery_used: bool,
    recovery_aim: (f32, f32),
    attack_drive: bool,
    back_air: bool,
    takeoff_y: f32,
    back_ground: bool,
    catch_before: i32,
    walk_motion: u64,
    walk_frame: f32,
    side_air: bool,
    dash_faced: bool,
    dash_lr: f32,
    air_lr: f32,
    air_dash_exit_speed: Option<f32>,
    dash_start_x: f32,
}
impl State {
    const NEW: Self = Self {
        object: u32::MAX,
        air_used: false,
        air_active: false,
        recovery_used: false,
        recovery_aim: (0.0, 0.0),
        attack_drive: false,
        back_air: false,
        takeoff_y: 0.0,
        back_ground: false,
        catch_before: 0,
        walk_motion: 0,
        walk_frame: 0.0,
        side_air: false,
        dash_faced: false,
        dash_lr: 1.0,
        air_lr: 1.0,
        air_dash_exit_speed: None,
        dash_start_x: 0.0,
    };
}
static mut STATES: [State; 8] = [State::NEW; 8];
pub(crate) unsafe fn entry(b: *mut app::BattleObjectModuleAccessor) -> Option<usize> {
    if b.is_null()
        || app::utility::get_kind(&mut *b) != *FIGHTER_KIND_CAPTAIN
        || !crate::is_slot(WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR))
    {
        return None;
    }
    let e = WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID);
    if !(0..8).contains(&e) {
        return None;
    }
    let i = e as usize;
    if STATES[i].object != (*b).battle_object_id {
        STATES[i] = State::NEW;
        STATES[i].object = (*b).battle_object_id;
        STATES[i].takeoff_y = PostureModule::pos_y(b);
    }
    Some(i)
}
unsafe fn previous(f: &mut L2CFighterCommon, line: smashline::StatusLine) -> L2CValue {
    type Lookup = unsafe extern "C" fn(
        &mut smash::lua2cpp::L2CFighterBase,
        smashline::StatusLine,
        i32,
    ) -> Option<std::ptr::NonNull<()>>;
    let mut address = 0usize;
    skyline::nn::ro::LookupSymbol(&mut address, b"smashline_get_original_status\0".as_ptr());
    if address == 0 {
        return 0.into();
    }
    let lookup: Lookup = std::mem::transmute(address);
    let status = StatusModule::status_kind(f.module_accessor);
    let base: &mut smash::lua2cpp::L2CFighterBase = std::mem::transmute(&mut *f);
    if let Some(pointer) = lookup(base, line, status) {
        let function: unsafe extern "C" fn(&mut L2CFighterCommon) -> L2CValue =
            std::mem::transmute(pointer.as_ptr());
        function(f)
    } else {
        0.into()
    }
}
pub(crate) unsafe fn facing(b: *mut app::BattleObjectModuleAccessor) -> f32 {
    let target = WorkModule::get_float(b, 0x4c);
    if target.is_finite() && target != 0.0 {
        target.signum()
    } else if PostureModule::lr(b) < 0.0 {
        -1.0
    } else {
        1.0
    }
}
pub(crate) unsafe fn speed(f: &mut L2CFighterCommon, x: f32) {
    speed_xy(f, x, 0.0);
}
unsafe fn speed_xy(f: &mut L2CFighterCommon, x: f32, y: f32) {
    let b = f.module_accessor;
    // STOP energy takes world-space velocity; model reflection and facing cannot reverse it.
    let air = StatusModule::situation_kind(b) != *SITUATION_KIND_GROUND;
    let kind = if air {
        *FIGHTER_KINETIC_TYPE_AIR_STOP
    } else {
        *FIGHTER_KINETIC_TYPE_GROUND_STOP
    };
    if KineticModule::get_kinetic_type(b) != kind {
        KineticModule::change_kinetic(b, kind);
    }
    if air {
        KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
    }
    KineticModule::clear_speed_all(b);
    KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_MOTION);
    KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_CONTROL);
    let reset = if StatusModule::situation_kind(b) == *SITUATION_KIND_GROUND {
        *ENERGY_STOP_RESET_TYPE_GROUND
    } else {
        *ENERGY_STOP_RESET_TYPE_AIR
    };
    sv_kinetic_energy!(
        reset_energy,
        f,
        FIGHTER_KINETIC_ENERGY_ID_STOP,
        reset,
        x,
        y,
        0.0,
        0.0,
        0.0
    );
    sv_kinetic_energy!(set_brake, f, FIGHTER_KINETIC_ENERGY_ID_STOP, 0.0, 0.0);
    sv_kinetic_energy!(set_accel, f, FIGHTER_KINETIC_ENERGY_ID_STOP, 0.0, 0.0);
    sv_kinetic_energy!(
        set_limit_speed,
        f,
        FIGHTER_KINETIC_ENERGY_ID_STOP,
        -1.0,
        -1.0
    );
    sv_kinetic_energy!(
        set_stable_speed,
        f,
        FIGHTER_KINETIC_ENERGY_ID_STOP,
        x.abs(),
        y.abs()
    );
    sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_STOP, x, y);
    KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_STOP);
}
pub(crate) unsafe fn release_drive(b: *mut app::BattleObjectModuleAccessor) {
    KineticModule::clear_speed_all(b);
    KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_STOP);
    KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_CONTROL);
    KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_MOTION);
}
pub(crate) unsafe fn settings(f: &mut L2CFighterCommon, air: bool) {
    StatusModule::init_settings(
        f.module_accessor,
        SituationKind(if air {
            *SITUATION_KIND_AIR
        } else {
            *SITUATION_KIND_GROUND
        }),
        if air {
            *FIGHTER_KINETIC_TYPE_MOTION_AIR
        } else {
            *FIGHTER_KINETIC_TYPE_MOTION
        },
        if air {
            *GROUND_CORRECT_KIND_AIR
        } else {
            *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP
        } as u32,
        GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT,
        0,
    );
}
unsafe extern "C" fn no_run(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let relative = ControlModule::get_stick_x(b) * facing(b);
    let next = if relative < -0.3 && WorkModule::get_float(b, 0x4c) != 0.0 {
        0x200
    } else if relative > 0.2 {
        *FIGHTER_STATUS_KIND_WALK
    } else {
        *FIGHTER_STATUS_KIND_WAIT
    };
    StatusModule::set_status_kind_interrupt(b, next);
    1.into()
}
unsafe extern "C" fn noop(_: &mut L2CFighterCommon) -> L2CValue {
    0.into()
}
unsafe extern "C" fn dash_pre(f: &mut L2CFighterCommon) -> L2CValue {
    settings(f, false);
    0.into()
}
unsafe extern "C" fn dash_main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return 0.into();
    };
    let lr = facing(b);
    STATES[e].dash_lr = lr;
    STATES[e].dash_start_x = PostureModule::pos_x(b);
    PostureModule::set_lr(b, lr);
    PostureModule::update_rot_y_lr(b);
    let back = super::movement_math::backward_input(ControlModule::get_stick_x(b), lr);

    STATES[e].back_ground = back;
    STATES[e].dash_faced = false;
    STATES[e].catch_before =
        WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_INVALID_CATCH_FRAME);
    WorkModule::unable_transition_term(b, *FIGHTER_STATUS_TRANSITION_TERM_ID_DASH_TO_RUN);
    if !back {
        WorkModule::enable_transition_term_group(
            b,
            *FIGHTER_STATUS_TRANSITION_GROUP_CHK_GROUND_JUMP,
        );
        WorkModule::enable_transition_term_group(
            b,
            *FIGHTER_STATUS_TRANSITION_GROUP_CHK_GROUND_SPECIAL,
        );
    }
    MotionModule::change_motion(
        b,
        Hash40::new(if back {
            "slayer_teleport_b"
        } else {
            "slayer_teleport_f"
        }),
        0.0,
        1.0,
        false,
        0.0,
        false,
        false,
    );
    set_dash_body_visible(b, false);
    if back {
        HitModule::set_whole(b, app::HitStatus(*HIT_STATUS_XLU), 0);
        WorkModule::set_int(
            b,
            STATES[e].catch_before.max(19),
            *FIGHTER_INSTANCE_WORK_ID_INT_INVALID_CATCH_FRAME,
        );
    }
    speed(f, if back { -1.0 * STATES[e].dash_lr } else { 0.0 });
    f.sub_shift_status_main(L2CValue::Ptr(dash_loop as *const () as _))
}
unsafe extern "C" fn dash_loop(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return 0.into();
    };
    let back = STATES[e].back_ground;
    if StatusModule::situation_kind(b) != *SITUATION_KIND_GROUND {
        f.change_status((*FIGHTER_STATUS_KIND_FALL).into(), false.into());
        return 1.into();
    }
    let fr = MotionModule::frame(b);
    // Enter through Side B's regular pre/main and let Dash End restore
    // visibility, intangibility, jostling, and kinetic state first.
    if !StopModule::is_stop(b)
        && ControlModule::get_command_flag_cat(b, 0) & *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_S != 0
        && can_start_side(f).get_bool()
    {
        ControlModule::clear_command_one(b, 0, *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_S);
        f.change_status((*FIGHTER_STATUS_KIND_SPECIAL_S).into(), false.into());
        return 1.into();
    }
    if fr >= 6.0 {
        WorkModule::unable_transition_term_group(
            b,
            *FIGHTER_STATUS_TRANSITION_GROUP_CHK_GROUND_JUMP,
        );
        WorkModule::unable_transition_term_group(
            b,
            *FIGHTER_STATUS_TRANSITION_GROUP_CHK_GROUND_SPECIAL,
        );
    }
    if !back
        && fr < 6.0
        && (f
            .sub_transition_group_check_ground_jump_mini_attack()
            .get_bool()
            || f.sub_transition_group_check_ground_jump().get_bool()
            || f.sub_transition_group_check_ground_special().get_bool())
    {
        return 1.into();
    }
    if back {
        HitModule::set_whole(
            b,
            app::HitStatus(if fr < 6.0 {
                *HIT_STATUS_XLU
            } else {
                *HIT_STATUS_NORMAL
            }),
            0,
        );
    }
    JostleModule::set_status(b, !(fr >= 6.0 && fr < if back { 19.0 } else { 11.0 }));
    let x = super::movement_math::teleport_velocity(fr, back, STATES[e].dash_lr);
    speed(f, x);
    if !STATES[e].dash_faced && fr >= if back { 19.0 } else { 11.0 } {
        let facing = WorkModule::get_float(b, 0x4c);
        if facing != 0.0 {
            PostureModule::set_lr(b, facing);
            PostureModule::update_rot_y_lr(b);
        }
        STATES[e].dash_faced = true;
    }
    if fr >= if back { 27.0 } else { 17.0 } {
        f.change_status((*FIGHTER_STATUS_KIND_WAIT).into(), false.into());
        return 1.into();
    }
    0.into()
}
unsafe extern "C" fn dash_end(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    if let Some(e) = entry(b) {

        if STATES[e].back_ground {
            HitModule::set_whole(b, app::HitStatus(*HIT_STATUS_NORMAL), 0);
            let elapsed = MotionModule::frame(b) as i32;
            WorkModule::set_int(
                b,
                (STATES[e].catch_before - elapsed).max(0),
                *FIGHTER_INSTANCE_WORK_ID_INT_INVALID_CATCH_FRAME,
            );
        }
    }
    set_dash_body_visible(b, true);
    let inverted = mirror_inverted(b);
    for name in ["Body_Outline", "Coat_Outline", "HeadLow_Outline"] {
        outline_pair(b, name, inverted, true);
    }
    for mesh in [
        "HeadLow_Base",
        "HeadLow_Eyes",
        "HeadLow_Glass",
        "headlow_base",
        "headlow_eyes",
        "headlow_glass",
    ] {
        ModelModule::set_mesh_visibility(b, Hash40::new(mesh), true);
    }
    JostleModule::set_status(b, true);
    release_drive(b);
    0.into()
}
const CHECK_SPECIAL_S_UNIQ: i32 = 0x39;
static mut ORIGINAL_SIDE_CHECK: [usize; 8] = [0; 8];

unsafe fn air_side_available(b: *mut app::BattleObjectModuleAccessor, entry: usize) -> bool {
    !STATES[entry].air_used && PostureModule::pos_y(b) - STATES[entry].takeoff_y >= 5.25
}

unsafe extern "C" fn can_start_side(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else { return false.into(); };
    if StatusModule::situation_kind(b) == *SITUATION_KIND_AIR && !air_side_available(b, e) {
        return false.into();
    }
    let original = ORIGINAL_SIDE_CHECK[e];
    if original != 0 && original != can_start_side as *const () as usize {
        let callback: unsafe extern "C" fn(&mut L2CFighterCommon) -> L2CValue = std::mem::transmute(original);
        return callback(f);
    }
    true.into()
}

unsafe extern "C" fn install_side_eligibility(f: &mut L2CFighterCommon) {
    let Some(e) = entry(f.module_accessor) else { return; };
    let slot = &mut f.global_table[CHECK_SPECIAL_S_UNIQ];
    let current = slot.try_get_ptr::<()>().map_or(0, |p| p as usize);
    if current != can_start_side as *const () as usize {
        ORIGINAL_SIDE_CHECK[e] = current;
        slot.assign(&L2CValue::Ptr(can_start_side as *const () as _));
    }
}

unsafe extern "C" fn air_s_pre(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else { return 0.into(); };
    let air = StatusModule::situation_kind(b) == *SITUATION_KIND_AIR;
    STATES[e].side_air = air;
    if !air {
        return super::dandy::pre(f);
    }
    if !air_side_available(b, e) {
        StatusModule::set_status_kind_interrupt(b, *FIGHTER_STATUS_KIND_FALL);
        return 1.into();
    }
    settings(f, true);
    0.into()
}
unsafe extern "C" fn air_s_main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else { return 0.into(); };
    let air = StatusModule::situation_kind(b) == *SITUATION_KIND_AIR;
    if air != STATES[e].side_air {
        let next = if air { *FIGHTER_STATUS_KIND_FALL } else { *FIGHTER_STATUS_KIND_LANDING };
        f.change_status(next.into(), false.into());
        return 1.into();
    }
    if !air {
        return super::dandy::main(f);
    }

    STATES[e].air_used = true;
    STATES[e].air_active = true;
    STATES[e].air_dash_exit_speed = None;
    let lr = facing(b);
    STATES[e].air_lr = lr;
    PostureModule::set_lr(b, lr);
    PostureModule::update_rot_y_lr(b);
    STATES[e].back_air = super::movement_math::backward_input(ControlModule::get_stick_x(b), lr);
    MotionModule::change_motion(
        b,
        Hash40::new(if STATES[e].back_air {
            "slayer_airdash_b"
        } else {
            "slayer_airdash_f"
        }),
        0.0,
        1.0,
        false,
        0.0,
        false,
        false,
    );
    AttackModule::clear_all(b);
    KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
    speed(f, 0.0);
    f.sub_shift_status_main(L2CValue::Ptr(air_s_loop as *const () as _))
}
unsafe extern "C" fn air_s_loop(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return 0.into();
    };
    if StatusModule::situation_kind(b) == *SITUATION_KIND_GROUND {
        f.change_status((*FIGHTER_STATUS_KIND_LANDING).into(), false.into());
        return 1.into();
    }
    if f.sub_transition_group_check_air_cliff().get_bool() {
        return 1.into();
    }
    if StopModule::is_stop(b) { return 0.into(); }
    let fr = MotionModule::frame(b) as i32;
    let back = STATES[e].back_air;
    if fr >= if back { 5 } else { 17 } {
        WorkModule::enable_transition_term_group(
            b,
            *FIGHTER_STATUS_TRANSITION_GROUP_CHK_AIR_ATTACK,
        );
        if f.sub_transition_group_check_air_attack().get_bool() {
            return 1.into();
        }
    }
    let boost = 0.34375 * 2.75 * 2.25 * 1.25;
    let v = if back {
        -0.98
    } else if fr < 12 {
        0.34375
    } else if fr < 13 {
        0.34375 * 2.75
    } else if fr < 15 {
        0.34375 * 2.75 * 2.25
    } else if fr < 18 {
        boost * 1.08
    } else {
        boost * 1.08 * 0.94_f32.powi(fr - 17)
    };
    let world_x = v * STATES[e].air_lr;
    speed(f, world_x);
    if MotionModule::is_end(b) {
        let air_speed = WorkModule::get_param_float(b, smash::hash40("air_speed_x_stable"), 0).max(0.0);
        STATES[e].air_dash_exit_speed = Some(world_x.clamp(-air_speed, air_speed));
        f.change_status((*FIGHTER_STATUS_KIND_FALL).into(), false.into());
        return 1.into();
    }
    0.into()
}
unsafe extern "C" fn air_s_end(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else { return 0.into(); };
    if STATES[e].side_air {
        if StatusModule::status_kind_next(b) != *FIGHTER_STATUS_KIND_FALL {
            STATES[e].air_dash_exit_speed = None;
        }
        if STATES[e].air_active {
            STATES[e].air_active = false;
            KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
            KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_CONTROL);
            KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_MOTION);
        }
        return 0.into();
    }
    super::dandy::end(f)
}
// Unmodified original Potemkin backward-walk callbacks use the engine's common walk-back machinery.
unsafe extern "C" fn back_pre(f: &mut L2CFighterCommon) -> L2CValue {
    super::mechanics::original_status(0xe134, f)
}
unsafe extern "C" fn back_init(f: &mut L2CFighterCommon) -> L2CValue {
    super::mechanics::original_status(0xe184, f)
}
unsafe extern "C" fn back_main(f: &mut L2CFighterCommon) -> L2CValue {

    let lr = facing(f.module_accessor);
    PostureModule::set_lr(f.module_accessor, lr);
    PostureModule::update_rot_y_lr(f.module_accessor);
    super::mechanics::original_status(0xe19c, f)
}
unsafe fn walk_velocity(f: &mut L2CFighterCommon, back: bool) {
    let b = f.module_accessor;
    if !StopModule::is_stop(b) {
        let stick = ControlModule::get_stick_x(b);
        let x = if stick.abs() > 0.2 {
            stick.signum() * if back { 0.24 } else { 0.28 }
        } else {
            0.0
        };
        speed(f, x);
        MotionModule::set_rate(b, 1.0);
    }
}
unsafe extern "C" fn back_exec(f: &mut L2CFighterCommon) -> L2CValue {
    let r = super::mechanics::original_status(0xe154, f);
    walk_velocity(f, true);
    r
}
unsafe extern "C" fn walk_exec(f: &mut L2CFighterCommon) -> L2CValue {
    let r = previous(f, smashline::StatusLine::Exec);
    walk_velocity(f, false);
    r
}
unsafe extern "C" fn back_end(f: &mut L2CFighterCommon) -> L2CValue {
    let result = super::mechanics::original_status(0xe114, f);
    release_drive(f.module_accessor);
    result
}
unsafe extern "C" fn back_exit(f: &mut L2CFighterCommon) -> L2CValue {
    super::mechanics::original_status(0xe16c, f)
}
unsafe extern "C" fn walk_end(f: &mut L2CFighterCommon) -> L2CValue {
    let r = previous(f, smashline::StatusLine::End);
    release_drive(f.module_accessor);
    r
}
unsafe fn select_jump(f: &mut L2CFighterCommon, air: bool) -> L2CValue {
    let b = f.module_accessor;
    let lr = facing(b);
    let input = ControlModule::get_stick_x(b);
    PostureModule::set_lr(b, lr);
    PostureModule::update_rot_y_lr(b);
    let result = previous(f, smashline::StatusLine::Main);
    use super::movement_math::JumpDirection;
    let choice = super::movement_math::jump_direction(input, lr);
    let motion = match (air, choice) {
        (false, JumpDirection::Neutral) => "jump_n",
        (false, JumpDirection::Forward) => "jump_f",
        (false, JumpDirection::Backward) => "jump_b",
        (true, JumpDirection::Neutral) => "jump_aerial_n",
        (true, JumpDirection::Forward) => "jump_aerial_f",
        (true, JumpDirection::Backward) => "jump_aerial_b",
    };
    MotionModule::change_motion(b, Hash40::new(motion), 0.0, 1.0, false, 0.0, false, false);
    if input.abs() <= 0.2 {
        sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_CONTROL, 0.0, 0.0);
        sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_STOP, 0.0, 0.0);
    }

    result
}
unsafe extern "C" fn jump_main(f: &mut L2CFighterCommon) -> L2CValue {
    select_jump(f, false)
}
unsafe extern "C" fn jump_air_main(f: &mut L2CFighterCommon) -> L2CValue {
    select_jump(f, true)
}
unsafe extern "C" fn back_brake(f: &mut L2CFighterCommon) -> L2CValue {
    KineticModule::clear_speed_all(f.module_accessor);
    f.change_status((*FIGHTER_STATUS_KIND_WAIT).into(), false.into());
    0.into()
}
// Keep the fighter root visible so the linked effect actor can draw the bats.
unsafe fn set_dash_body_visible(b: *mut app::BattleObjectModuleAccessor, visible: bool) {
    for name in ["Body_Base", "Body_Decal", "Coat_Base", "Coat_Decal", "body_base", "body_decal", "coat_base", "coat_decal"] {
        ModelModule::set_mesh_visibility(b, Hash40::new(name), visible);
    }
}

const RECOVERY_VANISH: f32 = 6.0;
const RECOVERY_REAPPEAR: f32 = 11.0;

unsafe extern "C" fn recovery_pre(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    if super::downb::attempting(b) { return super::downb::attempt_pre(f); }
    let Some(e) = entry(b) else { return 0.into(); };
    if StatusModule::situation_kind(b) == *SITUATION_KIND_AIR && STATES[e].recovery_used {
        StatusModule::set_status_kind_interrupt(b, *FIGHTER_STATUS_KIND_FALL_SPECIAL);
        return 1.into();
    }
    settings(f, true);
    0.into()
}

unsafe extern "C" fn recovery_main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    if super::downb::attempting(b) { return super::downb::attempt_main(f); }
    let Some(e) = entry(b) else { return 0.into(); };
    STATES[e].recovery_used = true;
    STATES[e].recovery_aim = recovery_aim(b);
    StatusModule::set_situation_kind(b, SituationKind(*SITUATION_KIND_AIR), true);
    GroundModule::correct(b, app::GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
    WorkModule::set_float(b, 20.0, *FIGHTER_INSTANCE_WORK_ID_FLOAT_LANDING_FRAME);
    MotionModule::change_motion(b, Hash40::new("slayer_teleport_f"), 0.0, 1.0, false, 0.0, false, false);
    speed_xy(f, 0.0, 0.1);
    f.sub_shift_status_main(L2CValue::Ptr(recovery_loop as *const () as _))
}

unsafe fn recovery_aim(b: *mut app::BattleObjectModuleAccessor) -> (f32, f32) {
    super::movement_math::recovery_velocity(ControlModule::get_stick_x(b), ControlModule::get_stick_y(b))
}

unsafe extern "C" fn recovery_loop(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else { return 0.into(); };
    let frame = MotionModule::frame(b);
    if frame >= RECOVERY_VANISH && StatusModule::situation_kind(b) == *SITUATION_KIND_GROUND {
        f.change_status((*FIGHTER_STATUS_KIND_LANDING_FALL_SPECIAL).into(), false.into());
        return 1.into();
    }
    if StopModule::is_stop(b) { return 0.into(); }
    let hidden = (RECOVERY_VANISH..RECOVERY_REAPPEAR).contains(&frame);
    JostleModule::set_status(b, !hidden);
    set_dash_body_visible(b, !hidden);
    if frame >= RECOVERY_REAPPEAR {
        GroundModule::set_cliff_check(b, GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_ALWAYS_BOTH_SIDES));
        if f.sub_transition_group_check_air_cliff().get_bool() { return 1.into(); }
    }
    if frame < RECOVERY_VANISH {
        STATES[e].recovery_aim = recovery_aim(b);
    }
    let (x, y) = if hidden {
        STATES[e].recovery_aim
    } else if frame < RECOVERY_VANISH {
        (0.0, 0.1)
    } else {
        let (aim_x, aim_y) = STATES[e].recovery_aim;
        let age = frame - RECOVERY_REAPPEAR;
        ((aim_x * 0.14).clamp(-1.2, 1.2) * 0.9_f32.powf(age), (aim_y * 0.12).min(1.15) - age * 0.15)
    };
    speed_xy(f, x, y);
    if MotionModule::is_end(b) || frame >= 17.0 {
        f.change_status((*FIGHTER_STATUS_KIND_FALL_SPECIAL).into(), false.into());
        return 1.into();
    }
    0.into()
}

unsafe extern "C" fn recovery_end(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    if super::downb::attempting(b) { return super::downb::attempt_end(f); }
    set_dash_body_visible(b, true);
    JostleModule::set_status(b, true);
    if StatusModule::status_kind_next(b) != *FIGHTER_STATUS_KIND_FALL_SPECIAL {
        KineticModule::clear_speed_all(b);
    }
    0.into()
}

unsafe fn attack_speed(motion: u64, frame: f32) -> Option<f32> {
    use super::movement_math::{attack_speed, Lunge};
    let attack = if motion == smash::hash40("slayer_2d") {
        Lunge::Sweep
    } else if motion == smash::hash40("slayer_fs") {
        Lunge::FarSlash
    } else if motion == smash::hash40("slayer_6h_tilt") {
        Lunge::ForwardHeavy
    } else if ["attack_s4_s", "attack_s4_hi", "attack_s4_lw"]
        .iter().any(|name| motion == smash::hash40(name))
    {
        Lunge::WildAssault
    } else {
        return None;
    };
    Some(attack_speed(attack, frame as i32))
}

pub(crate) unsafe fn teleport_active(b: *mut app::BattleObjectModuleAccessor) -> bool {
    if StatusModule::status_kind(b) == *FIGHTER_STATUS_KIND_SPECIAL_HI {
        return !super::downb::attempting(b)
            && (RECOVERY_VANISH..RECOVERY_REAPPEAR).contains(&MotionModule::frame(b));
    }
    let m = MotionModule::motion_kind(b);
    m == smash::hash40("slayer_teleport_f") || m == smash::hash40("slayer_teleport_b")
}
pub(crate) unsafe fn mirror_inverted(b: *mut app::BattleObjectModuleAccessor) -> bool {
    let st = StatusModule::status_kind(b);
    let m = MotionModule::motion_kind(b);
    PostureModule::lr(b) < 0.0
        && st != *FIGHTER_STATUS_KIND_TURN
        && st != 0x201
        && m != smash::hash40("turn")
        && m != smash::hash40("air_turn")
}
pub(crate) unsafe fn outline_pair(
    b: *mut app::BattleObjectModuleAccessor,
    name: &str,
    inverted: bool,
    visible: bool,
) {
    let (base, lower, mirror, mirror_lower) = match name {
        "Body_Outline" => ("Body_Outline", "body_outline", "Body_Outline_Mirror", "body_outline_mirror"),
        "Coat_Outline" => ("Coat_Outline", "coat_outline", "Coat_Outline_Mirror", "coat_outline_mirror"),
        "Coat_Back_Outline" => ("Coat_Back_Outline", "coat_back_outline", "Coat_Back_Outline_Mirror", "coat_back_outline_mirror"),
        "HeadLow_Outline" => ("HeadLow_Outline", "headlow_outline", "HeadLow_Outline_Mirror", "headlow_outline_mirror"),
        "Head_Outline" => ("Head_Outline", "head_outline", "Head_Outline_Mirror", "head_outline_mirror"),
        _ => return,
    };
    for (mesh, show) in [(base, !inverted), (lower, !inverted), (mirror, inverted), (mirror_lower, inverted)] {
        ModelModule::set_mesh_visibility(b, Hash40::new(mesh), show && visible);
    }
}
unsafe extern "C" fn frame(f: &mut L2CFighterCommon) {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return;
    };
    let st = StatusModule::status_kind(b);
    let motion = MotionModule::motion_kind(b);
    // Reflect the custom mesh and gameplay aliases together; choose matching outline winding.
    let inverted = mirror_inverted(b);
    let scale = PostureModule::scale(b);
    ModelModule::set_joint_scale(
        b,
        Hash40::new("top"),
        &Vector3f {
            x: if inverted { -scale } else { scale },
            y: scale,
            z: scale,
        },
    );
    // GGST hides the coat where the source clip has no coat track: 2K (clip 231) and 2S (clip 232).
    let teleport = teleport_active(b);
    if teleport {
        set_dash_body_visible(b, false);
    }
    let coat = !teleport && ![*FIGHTER_STATUS_KIND_SQUAT, *FIGHTER_STATUS_KIND_SQUAT_WAIT, *FIGHTER_STATUS_KIND_SQUAT_RV].contains(&st) && motion != smash::hash40("attack_lw3") && motion != smash::hash40("attack_s3_lw");
    let coat_back = coat && (motion != smash::hash40("slayer_2d") || MotionModule::frame(b) >= 21.0);
    outline_pair(b, "Body_Outline", inverted, !teleport_active(b));
    outline_pair(b, "Coat_Outline", inverted, !teleport_active(b) && coat);
    outline_pair(b, "Coat_Back_Outline", inverted, coat_back);
    for mesh in ["Coat_Back", "Coat_Back_Decal", "coat_back", "coat_back_decal"] {
        ModelModule::set_mesh_visibility(b, Hash40::new(mesh), coat_back);
    }
    for mesh in ["Coat_Base", "Coat_Decal", "coat_base", "coat_decal"] {
        ModelModule::set_mesh_visibility(b, Hash40::new(mesh), coat);
    }
    outline_pair(
        b,
        "HeadLow_Outline",
        inverted,
        super::cinematic::low_head_visible(b),
    );
    if StatusModule::situation_kind(b) == *SITUATION_KIND_GROUND {
        STATES[e].air_used = false;
        if st != *FIGHTER_STATUS_KIND_SPECIAL_HI { STATES[e].recovery_used = false; }
        STATES[e].takeoff_y = PostureModule::pos_y(b);
    }
    if !app::sv_information::is_ready_go() || StopModule::is_stop(b) {
        return;
    }
    if st != *FIGHTER_STATUS_KIND_SPECIAL_S {
        if let Some(exit_speed) = STATES[e].air_dash_exit_speed.take() {
            if st == *FIGHTER_STATUS_KIND_FALL
                && StatusModule::prev_status_kind(b, 0) == *FIGHTER_STATUS_KIND_SPECIAL_S
            {
                // Fall has initialized its own kinetics. Seed its normal air
                // control with the dash's world-space drift once.
                sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_CONTROL, exit_speed, 0.0);
            }
        }
    }
    let velocity = attack_speed(motion, MotionModule::frame(b))
        .filter(|_| StatusModule::situation_kind(b) == *SITUATION_KIND_GROUND);
    if let Some(x) = velocity {
        if x > 0.0 || STATES[e].attack_drive {
            speed(f, x * PostureModule::lr(b));
            STATES[e].attack_drive = x > 0.0;
        }
    } else {
        // The next status owns its own kinetic energy, including damage and jumps.
        STATES[e].attack_drive = false;
    }
    if [*FIGHTER_STATUS_KIND_WAIT, *FIGHTER_STATUS_KIND_WALK].contains(&st)
        && WorkModule::get_float(b, 0x4c) != 0.0
        && ControlModule::get_stick_x(b) * facing(b) < -0.3
        && ControlModule::get_stick_y(b) > -0.5
        && !ControlModule::check_button_on(b, *CONTROL_PAD_BUTTON_GUARD)
        && !ControlModule::check_button_on(b, *CONTROL_PAD_BUTTON_CATCH)
    {
        let command = ControlModule::get_command_flag_cat(b, 0);
        if command
            & (*FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_N
                | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_S3
                | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_HI3
                | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_LW3
                | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_S4
                | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_HI4
                | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_LW4
                | *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_N
                | *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_S
                | *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_HI
                | *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_LW
                | *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON
                | *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP)
            == 0
        {
            StatusModule::change_status_request(b, 0x200, false);
        }
    }
    if st == *FIGHTER_STATUS_KIND_WALK || st == 0x200 {
        walk_velocity(f, st == 0x200);
        let fr = MotionModule::frame(b);
        if STATES[e].walk_motion == motion && fr + 1.0 < STATES[e].walk_frame {
            MotionModule::set_frame(b, if st == 0x200 { 70.0 } else { 12.0 }, true);
        }
        STATES[e].walk_motion = motion;
        STATES[e].walk_frame = MotionModule::frame(b);
    } else {
        STATES[e].walk_motion = 0;
        STATES[e].walk_frame = 0.0;
    }
}
unsafe extern "C" fn movement_acmd(_: &mut smash::lua2cpp::L2CAgentBase) {}
pub fn install() {
    unsafe {
        param_config::update_float(
            *FIGHTER_KIND_CAPTAIN,
            crate::param_slots(),
            (smash::hash40("walk_speed_max"), 0),
            0.28,
        );
        param_config::update_float(
            *FIGHTER_KIND_CAPTAIN,
            crate::param_slots(),
            (smash::hash40("walk_speed_back_mul"), 0),
            0.24 / 0.28,
        );
    }
    Agent::new("captain")
        .set_costume(crate::slots())
        .status(Pre, *FIGHTER_STATUS_KIND_RUN, no_run)
        .status(Pre, *FIGHTER_STATUS_KIND_TURN_RUN, no_run)
        .status(Pre, *FIGHTER_STATUS_KIND_DASH, dash_pre)
        .status(Init, *FIGHTER_STATUS_KIND_DASH, noop)
        .status(Exec, *FIGHTER_STATUS_KIND_DASH, noop)
        .status(Exit, *FIGHTER_STATUS_KIND_DASH, noop)
        .status(Main, *FIGHTER_STATUS_KIND_DASH, dash_main)
        .status(End, *FIGHTER_STATUS_KIND_DASH, dash_end)
        .status(Pre, *FIGHTER_STATUS_KIND_TURN_DASH, dash_pre)
        .status(Init, *FIGHTER_STATUS_KIND_TURN_DASH, noop)
        .status(Exec, *FIGHTER_STATUS_KIND_TURN_DASH, noop)
        .status(Exit, *FIGHTER_STATUS_KIND_TURN_DASH, noop)
        .status(Main, *FIGHTER_STATUS_KIND_TURN_DASH, dash_main)
        .status(End, *FIGHTER_STATUS_KIND_TURN_DASH, dash_end)
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_HI, recovery_pre)
        .status(Init, *FIGHTER_STATUS_KIND_SPECIAL_HI, noop)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_HI, recovery_main)
        .status(Exec, *FIGHTER_STATUS_KIND_SPECIAL_HI, noop)
        .status(Exit, *FIGHTER_STATUS_KIND_SPECIAL_HI, noop)
        .status(End, *FIGHTER_STATUS_KIND_SPECIAL_HI, recovery_end)
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_S, air_s_pre)
        .status(Main, *FIGHTER_STATUS_KIND_SPECIAL_S, air_s_main)
        .status(End, *FIGHTER_STATUS_KIND_SPECIAL_S, air_s_end)
        .status(Init, *FIGHTER_STATUS_KIND_SPECIAL_S, noop)
        .status(Exec, *FIGHTER_STATUS_KIND_SPECIAL_S, noop)
        .status(Exit, *FIGHTER_STATUS_KIND_SPECIAL_S, noop)
        .status(Exec, *FIGHTER_STATUS_KIND_WALK, walk_exec)
        .status(End, *FIGHTER_STATUS_KIND_WALK, walk_end)
        .status(Main, *FIGHTER_STATUS_KIND_JUMP, jump_main)
        .status(Main, *FIGHTER_STATUS_KIND_JUMP_AERIAL, jump_air_main)
        .status(Pre, 0x200, back_pre)
        .status(Init, 0x200, back_init)
        .status(Main, 0x200, back_main)
        .status(Exec, 0x200, back_exec)
        .status(End, 0x200, back_end)
        .status(Exit, 0x200, back_exit)
        .status(Pre, 0x202, dash_pre)
        .status(Main, 0x202, back_brake)
        .game_acmd("game_slayerteleportf", movement_acmd, Priority::High)
        .effect_acmd("effect_slayerteleportf", movement_acmd, Priority::High)
        .sound_acmd("sound_slayerteleportf", movement_acmd, Priority::High)
        .expression_acmd("expression_slayerteleportf", movement_acmd, Priority::High)
        .game_acmd("game_slayerteleportb", movement_acmd, Priority::High)
        .effect_acmd("effect_slayerteleportb", movement_acmd, Priority::High)
        .sound_acmd("sound_slayerteleportb", movement_acmd, Priority::High)
        .expression_acmd("expression_slayerteleportb", movement_acmd, Priority::High)
        .game_acmd("game_slayerairdashf", movement_acmd, Priority::High)
        .effect_acmd("effect_slayerairdashf", movement_acmd, Priority::High)
        .sound_acmd("sound_slayerairdashf", movement_acmd, Priority::High)
        .expression_acmd("expression_slayerairdashf", movement_acmd, Priority::High)
        .game_acmd("game_slayerairdashb", movement_acmd, Priority::High)
        .effect_acmd("effect_slayerairdashb", movement_acmd, Priority::High)
        .sound_acmd("sound_slayerairdashb", movement_acmd, Priority::High)
        .expression_acmd("expression_slayerairdashb", movement_acmd, Priority::High)
        .on_start(install_side_eligibility)
        .on_line(Main, install_side_eligibility)
        .on_line(Main, frame)
        .install();
}
