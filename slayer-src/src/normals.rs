//! v0.3.18: source normal selection and GGST gatling cancels.
//! Side tilt back (relative to the opponent) is f.S; down tilt forward/back is 2S/2D. On hit or block a normal
//! cancels into its source AddChain routes, ground normals (except 6H) into Mappa/Dandy, 5K/c.S into jump or dash,
//! and j.K/j.S/j.H/j.D into a double jump or forward air dash (v0.3.77, Dustloop gatling table).
//! Inputs are buffered for a few frames and applied once hitstop ends, only while the same attack is still running
//! (so Roman Cancel or any other status change wins).
use super::normals_math::{self as nm, Cancel, Normal};
use smash::{
    app::{self, lua_bind::*},
    lib::lua_const::*,
    lua2cpp::L2CFighterCommon,
    phx::Hash40,
};
use smashline::{Agent, Main};
const BUFFER: i32 = 8;
#[derive(Clone, Copy)]
struct State {
    status: i32,
    frame: f32,
    lr: f32,
    command: i32,
    connected: bool,
    pending: Option<(Cancel, i32)>,
    forced: Option<(i32, u64, i32)>,
}
static mut STATES: [State; 8] = [State {
    status: -1,
    frame: 0.0,
    lr: 1.0,
    command: 0,
    connected: false,
    pending: None,
    forced: None,
}; 8];
#[derive(Clone, Copy)]
struct Contacts {
    owner: u32,
    motion: u64,
    frame: f32,
    targets: [u32; 8],
}
impl Contacts {
    const EMPTY: Self = Self { owner: u32::MAX, motion: 0, frame: 0.0, targets: [u32::MAX; 8] };
}
static mut CONTACTS: [Contacts; 8] = [Contacts::EMPTY; 8];

pub(crate) unsafe fn record_contact(
    attacker: *mut app::BattleObjectModuleAccessor,
    victim: *mut app::BattleObjectModuleAccessor,
) {
    let Some(e) = super::locomotion::entry(attacker) else { return };
    if current(attacker).is_none() {
        return;
    }
    let id = WorkModule::get_int(victim, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID);
    if !(0..8).contains(&id) {
        return;
    }
    let owner = (*attacker).battle_object_id;
    let motion = MotionModule::motion_kind(attacker);
    let frame = MotionModule::frame(attacker);
    let contacts = &mut CONTACTS[e];
    if contacts.owner != owner || contacts.motion != motion || frame + 0.5 < contacts.frame {
        *contacts = Contacts { owner, motion, frame, ..Contacts::EMPTY };
    }
    contacts.frame = frame;
    contacts.targets[id as usize] = (*victim).battle_object_id;
}

unsafe fn victim_in_hitstop(b: *mut app::BattleObjectModuleAccessor, e: usize) -> bool {
    let contacts = CONTACTS[e];
    if contacts.owner != (*b).battle_object_id
        || contacts.motion != MotionModule::motion_kind(b)
        || MotionModule::frame(b) + 0.5 < contacts.frame
    {
        return false;
    }
    contacts.targets.into_iter().any(|id| {
        if id == u32::MAX || !app::sv_battle_object::is_active(id) {
            return false;
        }
        let target = app::sv_battle_object::module_accessor(id);
        !target.is_null() && StopModule::is_stop(target)
    })
}
unsafe fn current(b: *mut app::BattleObjectModuleAccessor) -> Option<Normal> {
    let k = MotionModule::motion_kind(b);
    let is = |n: &str| k == Hash40::new(n).hash;
    Some(if is("attack_11") {
        Normal::P5
    } else if is("attack_s3_s") || is("attack_s3_hi") {
        Normal::K5
    } else if is("attack_s3_lw") {
        Normal::S2
    } else if is("slayer_fs") {
        Normal::FS
    } else if is("attack_hi3") {
        Normal::P6
    } else if is("attack_lw3") {
        Normal::K2
    } else if is("slayer_2d") {
        Normal::D2
    } else if is("slayer_6h_tilt") {
        Normal::H6
    } else if is("attack_hi4") {
        Normal::CS
    } else if is("attack_lw4") {
        Normal::H2
    } else if is("attack_air_n") {
        Normal::JP
    } else if is("attack_air_f") {
        Normal::JK
    } else if is("attack_air_b") {
        Normal::JH
    } else if is("attack_air_hi") {
        Normal::JD
    } else if is("attack_air_lw") {
        Normal::JS
    } else {
        return None;
    })
}
/// Status and motion that start a normal; None keeps whatever motion the status picks.
fn start(n: Normal) -> (i32, Option<&'static str>) {
    unsafe {
        match n {
            Normal::P5 => (*FIGHTER_STATUS_KIND_ATTACK, Some("attack_11")),
            Normal::K5 => (*FIGHTER_STATUS_KIND_ATTACK_S3, Some("attack_s3_s")),
            Normal::FS => (*FIGHTER_STATUS_KIND_ATTACK_S3, Some("slayer_fs")),
            Normal::S2 => (*FIGHTER_STATUS_KIND_ATTACK_LW3, Some("attack_s3_lw")),
            Normal::K2 => (*FIGHTER_STATUS_KIND_ATTACK_LW3, Some("attack_lw3")),
            Normal::D2 => (*FIGHTER_STATUS_KIND_ATTACK_LW3, Some("slayer_2d")),
            Normal::P6 => (*FIGHTER_STATUS_KIND_ATTACK_HI3, Some("attack_hi3")),
            Normal::H6 => (*FIGHTER_STATUS_KIND_ATTACK_HI3, Some("slayer_6h_tilt")),
            Normal::CS => (*FIGHTER_STATUS_KIND_ATTACK_HI4_START, None),
            Normal::H2 => (*FIGHTER_STATUS_KIND_ATTACK_LW4_START, None),
            Normal::JP => (*FIGHTER_STATUS_KIND_ATTACK_AIR, Some("attack_air_n")),
            Normal::JK => (*FIGHTER_STATUS_KIND_ATTACK_AIR, Some("attack_air_f")),
            Normal::JH => (*FIGHTER_STATUS_KIND_ATTACK_AIR, Some("attack_air_b")),
            Normal::JD => (*FIGHTER_STATUS_KIND_ATTACK_AIR, Some("attack_air_hi")),
            Normal::JS => (*FIGHTER_STATUS_KIND_ATTACK_AIR, Some("attack_air_lw")),
        }
    }
}
unsafe fn tilt_stick(b: *mut app::BattleObjectModuleAccessor) -> (f32, f32) {
    if ControlModule::check_button_on(b, *CONTROL_PAD_BUTTON_CSTICK_ON) {
        (ControlModule::get_sub_stick_x(b), ControlModule::get_sub_stick_y(b))
    } else {
        (ControlModule::get_stick_x(b), ControlModule::get_stick_y(b))
    }
}
unsafe fn requested(b: *mut app::BattleObjectModuleAccessor, fresh: i32, facing: f32) -> Option<Cancel> {
    let air = StatusModule::situation_kind(b) != *SITUATION_KIND_GROUND;
    let attack = *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_N
        | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_S3
        | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_HI3
        | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_LW3
        | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_S4
        | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_HI4
        | *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_LW4;
    if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_S != 0 && !air {
        return Some(Cancel::SpecialS);
    }
    if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_N != 0 && !air {
        return Some(Cancel::SpecialN);
    }
    if air {
        // Dustloop: j.K/j.S/j.H/j.D cancel into a jump or a forward air dash on hit or block.
        if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_S != 0 {
            let x = if ControlModule::check_button_on(b, *CONTROL_PAD_BUTTON_CSTICK_ON) {
                ControlModule::get_sub_stick_x(b)
            } else {
                ControlModule::get_stick_x(b)
            };
            return if x * facing > 0.2 { Some(Cancel::Dash) } else { None };
        }
        if fresh & (*FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON | *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP) != 0 {
            return Some(Cancel::Jump);
        }
        if fresh & attack == 0 {
            return None;
        }
        let kind = ControlModule::get_attack_air_kind(b);
        return Some(Cancel::Normal(if kind == *FIGHTER_COMMAND_ATTACK_AIR_KIND_F {
            Normal::JK
        } else if kind == *FIGHTER_COMMAND_ATTACK_AIR_KIND_B {
            Normal::JH
        } else if kind == *FIGHTER_COMMAND_ATTACK_AIR_KIND_HI {
            Normal::JD
        } else if kind == *FIGHTER_COMMAND_ATTACK_AIR_KIND_LW {
            Normal::JS
        } else {
            Normal::JP
        }));
    }
    let (x, y) = tilt_stick(b);
    let n = if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_HI4 != 0 {
        Normal::CS
    } else if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_LW4 != 0 {
        Normal::H2
    } else if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_HI3 != 0 {
        nm::up_tilt(x, facing)
    } else if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_LW3 != 0 {
        nm::down_tilt(x, facing)
    } else if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_S3 != 0 {
        if y > 0.3 { nm::up_tilt(x, facing) } else { nm::side_tilt(x, y, facing) }
    } else if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_N != 0 {
        Normal::P5
    } else if fresh & (*FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON | *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP) != 0 {
        return Some(Cancel::Jump);
    } else if fresh & *FIGHTER_PAD_CMD_CAT1_FLAG_DASH != 0 && x * facing > 0.0 {
        return Some(Cancel::Dash);
    } else {
        return None;
    };
    Some(Cancel::Normal(n))
}
unsafe fn force_motion(b: *mut app::BattleObjectModuleAccessor, motion: u64) {
    if MotionModule::motion_kind(b) != motion {
        MotionModule::change_motion(b, Hash40::new_raw(motion), 0.0, 1.0, false, 0.0, false, false);
    }
}
unsafe extern "C" fn frame(f: &mut L2CFighterCommon) {
    let b = f.module_accessor;
    let Some(e) = super::locomotion::entry(b) else {
        return;
    };
    let st = &mut STATES[e];
    let status = StatusModule::status_kind(b);
    let motion_frame = MotionModule::frame(b);
    let started = status != st.status || motion_frame + 0.5 < st.frame;
    let target = WorkModule::get_float(b, 0x4c);
    let facing = if target.is_finite() && target != 0.0 { target.signum() } else { st.lr };
    let command = ControlModule::get_command_flag_cat(b, 0);
    let fresh = command & !st.command;
    st.command = command;
    if started {
        st.connected = false;
        st.pending = None;
        if let Some((kind, motion, frames)) = st.forced {
            if kind == status {
                force_motion(b, motion);
                st.forced = if frames > 0 { Some((kind, motion, frames - 1)) } else { None };
            } else {
                st.forced = None;
            }
        } else {
            let (x, y) = tilt_stick(b);
            let selected = if status == *FIGHTER_STATUS_KIND_ATTACK_S3 {
                if y > 0.3 { Some(nm::up_tilt(x, facing)) }
                else { Some(nm::side_tilt(x, y, facing)) }
            } else if status == *FIGHTER_STATUS_KIND_ATTACK_LW3 {
                Some(nm::down_tilt(x, facing))
            } else if status == *FIGHTER_STATUS_KIND_ATTACK_HI3 {
                Some(nm::up_tilt(x, facing))
            } else {
                None
            };
            if let Some(n @ (Normal::FS | Normal::S2 | Normal::D2 | Normal::P6 | Normal::H6)) = selected {
                if let (_, Some(motion)) = start(n) {
                    PostureModule::set_lr(b, facing);
                    PostureModule::update_rot_y_lr(b);
                    force_motion(b, Hash40::new(motion).hash);
                }
            }
        }
    } else if let Some((kind, motion, frames)) = st.forced {
        // The status may still pick its own motion on its first frames; keep the requested one.
        if kind == status && motion_frame < 2.0 {
            force_motion(b, motion);
        }
        st.forced = if frames > 0 { Some((kind, motion, frames - 1)) } else { None };
    }
    st.status = status;
    st.frame = MotionModule::frame(b);
    st.lr = PostureModule::lr(b);
    let Some(from) = current(b) else {
        return;
    };
    if AttackModule::is_infliction_status(b, *COLLISION_KIND_MASK_HIT | *COLLISION_KIND_MASK_SHIELD) {
        st.connected = true;
    }
    if !ControlModule::check_button_on(b, *CONTROL_PAD_BUTTON_GUARD) && !ControlModule::check_button_on(b, *CONTROL_PAD_BUTTON_CATCH) {
        if let Some(c) = requested(b, fresh, facing) {
            if nm::allowed(from, c) {
                st.pending = Some((c, BUFFER));
            }
        }
    }
    let Some((cancel, life)) = st.pending else {
        return;
    };
    if !st.connected {
        st.pending = if life > 0 { Some((cancel, life - 1)) } else { None };
        return;
    }
    if StopModule::is_stop(b) || victim_in_hitstop(b, e) {
        return;
    }
    st.pending = None;
    st.connected = false;
    let kind = match cancel {
        Cancel::Normal(n) => {
            let (kind, motion) = start(n);
            st.forced = motion.map(|m| (kind, Hash40::new(m).hash, 2));
            if n == Normal::FS {
                PostureModule::set_lr(b, facing);
                PostureModule::update_rot_y_lr(b);
            }
            kind
        }
        Cancel::SpecialN => *FIGHTER_STATUS_KIND_SPECIAL_N,
        Cancel::SpecialS => *FIGHTER_STATUS_KIND_SPECIAL_S,
        Cancel::Jump if nm::grounded(from) => *FIGHTER_STATUS_KIND_JUMP_SQUAT,
        Cancel::Jump => {
            let used = WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_JUMP_COUNT);
            let max = WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_JUMP_COUNT_MAX);
            if used >= max {
                return;
            }
            *FIGHTER_STATUS_KIND_JUMP_AERIAL
        }
        Cancel::Dash if nm::grounded(from) => *FIGHTER_STATUS_KIND_DASH,
        // Air Side B is Slayer's air dash; its pre-status guard still enforces once per airtime.
        Cancel::Dash => *FIGHTER_STATUS_KIND_SPECIAL_S,
    };
    StatusModule::change_status_request_from_script(b, kind, false);

}
pub fn install() {
    Agent::new("captain").set_costume(crate::slots()).on_line(Main, frame).install();
}
