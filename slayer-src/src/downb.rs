//! Bloodsucking Universe and air bite use Falcon's native special-capture handshake.
use super::locomotion::{entry, facing};
use smash::{
    app::{self, lua_bind::*, GroundCliffCheckKind, SituationKind},
    lib::{lua_const::*, L2CValue},
    lua2cpp::{L2CAgentBase, L2CFighterCommon},
    phx::{Hash40, Vector3f},
};
use smash_script::{lua_args, macros, sv_kinetic_energy};
use smashline::{Agent, End, Exec, Exit, Init, Main, Pre, Priority};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

const NO_TARGET: u32 = 0x50000000;
const BITE_DAMAGE: f32 = 14.5;
const UNIVERSE_POWER: f32 = 1.2;
const LINK_DEPTH: usize = 0x488df0;
static ORIGINAL_DEPTH: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_DEPTH: [AtomicUsize; 8] = [const { AtomicUsize::new(0) }; 8];
static ACTIVE_GRABBERS: [AtomicU32; 8] = [const { AtomicU32::new(NO_TARGET) }; 8];
#[derive(Clone, Copy)]
struct Grab {
    airborne: bool,
    target: u32,
    wait_ticks: u8,
    holding: bool,
    released: bool,
    stepped: bool,
    bounced: bool,
}

impl Grab {
    fn new(airborne: bool) -> Self {
        Self {
            airborne, target: NO_TARGET, wait_ticks: 0, holding: false,
            released: false, stepped: false, bounced: false,
        }
    }
}

#[derive(Clone, Copy)]
struct State {
    grab: Option<Grab>,
    buff: bool,
    buff_age: u32,
    buff_end_age: Option<u8>,
    powered: bool,
    object: u32,
}

impl State {
    const EMPTY: Self = Self { grab: None, buff: false, buff_age: 0, buff_end_age: None, powered: false, object: u32::MAX };
}

static mut STATES: [State; 8] = [State::EMPTY; 8];

unsafe fn state_entry(b: *mut app::BattleObjectModuleAccessor) -> Option<usize> {
    let e = entry(b)?;
    let object = (*b).battle_object_id;
    if STATES[e].object != object {
        ACTIVE_GRABBERS[e].store(NO_TARGET, Ordering::Release);
        STATES[e] = State { object, ..State::EMPTY };
    }
    Some(e)
}

fn active_grabber(object: u32) -> Option<usize> {
    if object == NO_TARGET {
        return None;
    }
    ACTIVE_GRABBERS.iter().position(|slot| slot.load(Ordering::Acquire) == object)
}

unsafe fn capture_depth_slot(b: *mut app::BattleObjectModuleAccessor) -> Option<usize> {
    if b.is_null() || app::utility::get_category(&mut *b) != *BATTLE_OBJECT_CATEGORY_FIGHTER {
        return None;
    }
    if let Some(e) = active_grabber((*b).battle_object_id) {
        return Some(e);
    }
    if !LinkModule::is_link(b, *LINK_NO_CAPTURE) {
        return None;
    }
    let parent = LinkModule::get_parent_id(b, *LINK_NO_CAPTURE, false) as u32;
    active_grabber(parent)
}

unsafe extern "C" fn capture_depth(link: *mut u8) -> f32 {
    let address = ORIGINAL_DEPTH.load(Ordering::Acquire);
    if address == 0 { return 0.0; }
    let original: unsafe extern "C" fn(*mut u8) -> f32 =
        std::mem::transmute(address);
    if ACTIVE_GRABBERS.iter().all(|slot| slot.load(Ordering::Relaxed) == NO_TARGET) {
        return original(link);
    }
    // The native implementation reads its own module accessor from LinkModule+8.
    let b = *link.add(8).cast::<*mut app::BattleObjectModuleAccessor>();
    let Some(e) = capture_depth_slot(b) else { return original(link) };
    let key = link as usize;
    match ACTIVE_DEPTH[e].compare_exchange(0, key, Ordering::Acquire, Ordering::Relaxed) {
        Ok(_) => {
            let depth = original(link);
            ACTIVE_DEPTH[e].store(0, Ordering::Release);
            depth
        }
        // Only a repeated query for the same object in this capture uses local depth.
        Err(active) if active == key => PostureModule::pos_z(b),
        Err(_) => original(link),
    }
}

unsafe fn install_capture_depth_guard() {
    let base = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize;
    let mut anchor = 0;
    skyline::nn::ro::LookupSymbol(
        &mut anchor, b"_ZN3app8lua_bind30LinkModule__get_parent_lr_implEPNS_26BattleObjectModuleAccessorEi\0".as_ptr(),
    );
    if anchor != base + 0x2048280
        || std::slice::from_raw_parts((base + LINK_DEPTH) as *const u32, 4)
            != [0xf81d0ff5, 0xa9014ff4, 0xa9027bfd, 0x910083fd]
        || std::slice::from_raw_parts((base + 0x488ee4) as *const u32, 4)
            != [0xf9406900, 0xf9400008, 0xf9428908, 0xd63f0100]
    {
        skyline::println!("[Slayer v0.3.69] capture depth guard unavailable");
        return;
    }
    let mut original = std::ptr::null_mut();
    skyline::hooks::A64HookFunction((base + LINK_DEPTH) as *const _, capture_depth as *const _, &mut original);
    ORIGINAL_DEPTH.store(original as usize, Ordering::Release);
    skyline::println!("[Slayer v0.3.69] capture depth guard installed");
}

unsafe fn air(b: *mut app::BattleObjectModuleAccessor) -> bool {
    StatusModule::situation_kind(b) == *SITUATION_KIND_AIR
}

unsafe fn prep(f: &mut L2CFighterCommon, airborne: bool, held: bool) -> L2CValue {
    let kinetic = match (airborne, held) {
        (true, true) => *FIGHTER_KINETIC_TYPE_AIR_STOP,
        (true, false) => *FIGHTER_KINETIC_TYPE_FALL,
        (false, _) => *FIGHTER_KINETIC_TYPE_GROUND_STOP,
    };
    StatusModule::init_settings(
        f.module_accessor,
        SituationKind(if airborne && held { *SITUATION_KIND_AIR } else { *SITUATION_KIND_NONE }),
        kinetic,
        if airborne { *GROUND_CORRECT_KIND_AIR } else { *GROUND_CORRECT_KIND_GROUND_CLIFF_STOP } as u32,
        GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE), true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLAG, *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_NONE_FLOAT,
        if held { *FS_SUCCEEDS_KEEP_ATTACK_ABSOLUTE } else { 0 },
    );
    FighterStatusModuleImpl::set_fighter_status_data(
        f.module_accessor, false, *FIGHTER_TREADED_KIND_ENABLE, false, true, false,
        (*FIGHTER_LOG_MASK_FLAG_ATTACK_KIND_SPECIAL_LW | *FIGHTER_LOG_MASK_FLAG_ACTION_CATEGORY_ATTACK) as u64,
        (*FIGHTER_STATUS_ATTR_DISABLE_JUMP_BOARD_EFFECT | *FIGHTER_STATUS_ATTR_DISABLE_TURN_DAMAGE) as u32,
        *FIGHTER_POWER_UP_ATTACK_BIT_SPECIAL_LW as u32, 0,
    );
    0.into()
}

unsafe extern "C" fn lw_pre(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let airborne = air(b);
    let next = if let Some(e) = state_entry(b) {
        STATES[e].grab = Some(Grab::new(airborne));
        ACTIVE_GRABBERS[e].store((*b).battle_object_id, Ordering::Release);
        // The capture handler checks the actual status, so enter Falcon's
        // special-grab startup before any motion or collision is initialized.
        *FIGHTER_STATUS_KIND_SPECIAL_HI
    } else if airborne {
        *FIGHTER_STATUS_KIND_FALL
    } else {
        *FIGHTER_STATUS_KIND_WAIT
    };
    StatusModule::set_status_kind_interrupt(b, next);
    1.into()
}

pub(crate) unsafe fn attempting(b: *mut app::BattleObjectModuleAccessor) -> bool {
    state_entry(b).is_some_and(|e| STATES[e].grab.is_some_and(|grab| !grab.holding))
}

pub(crate) unsafe fn attempt_pre(f: &mut L2CFighterCommon) -> L2CValue {
    prep(f, air(f.module_accessor), false)
}

unsafe extern "C" fn noop(_: &mut L2CFighterCommon) -> L2CValue { 0.into() }

unsafe fn motion(b: *mut app::BattleObjectModuleAccessor, name: &str) {
    MotionModule::change_motion(b, Hash40::new(name), 0.0, 1.0, false, 0.0, false, false);
}

pub(crate) unsafe fn attempt_main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(grab) = state_entry(b).and_then(|e| STATES[e].grab) else { return recover(b, air(b)) };
    let airborne = grab.airborne;
    PostureModule::set_lr(b, facing(b));
    PostureModule::update_rot_y_lr(b);
    motion(b, if airborne { "slayer_airbite" } else { "slayer_bu" });
    skyline::println!("[Slayer Down B] attempt owner={} air={}", (*b).battle_object_id, airborne);
    f.sub_shift_status_main(L2CValue::Ptr(attempt_loop as *const () as _))
}

unsafe fn recover(b: *mut app::BattleObjectModuleAccessor, started_air: bool) -> L2CValue {
    let status = if air(b) {
        *FIGHTER_STATUS_KIND_FALL
    } else if started_air {
        *FIGHTER_STATUS_KIND_LANDING
    } else {
        *FIGHTER_STATUS_KIND_WAIT
    };
    StatusModule::change_status_request_from_script(b, status, true);
    1.into()
}

unsafe extern "C" fn attempt_loop(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(grab) = state_entry(b).and_then(|e| STATES[e].grab) else { return recover(b, air(b)) };
    if StatusModule::status_kind_next(b) == *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING {
        return 0.into();
    }
    if grab.airborne != air(b) || MotionModule::is_end(b) {
        return recover(b, grab.airborne);
    }
    0.into()
}

pub(crate) unsafe fn attempt_end(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    if StatusModule::status_kind_next(b) != *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING {
        GrabModule::clear_all(b);
        end_capture(b, true);
    }
    0.into()
}

unsafe fn linked_victim(
    b: *mut app::BattleObjectModuleAccessor, target: u32,
) -> Option<*mut app::BattleObjectModuleAccessor> {
    if target == NO_TARGET || target == (*b).battle_object_id || !app::sv_battle_object::is_active(target)
        || !LinkModule::is_linked(b, *LINK_NO_CAPTURE)
        || LinkModule::get_node_object_id(b, *LINK_NO_CAPTURE) as u32 != target
    {
        return None;
    }
    let victim = app::sv_battle_object::module_accessor(target);
    if victim.is_null() || !LinkModule::is_link(victim, *LINK_NO_CAPTURE)
        // The direct ID read avoids dereferencing the parent's cached object pointer.
        || LinkModule::get_parent_id(victim, *LINK_NO_CAPTURE, false) as u32 != (*b).battle_object_id
    {
        return None;
    }
    Some(victim)
}

unsafe fn space_victim(victim: *mut app::BattleObjectModuleAccessor) {
    // Retarget the native capture without creating a second constraint.
    let anchor = Hash40::new("throw");
    if LinkModule::is_model_constraint(victim)
        && LinkModule::get_model_constraint_no(victim) as i32 == *LINK_NO_CAPTURE
        && LinkModule::get_model_constraint_target_joint(victim) != anchor.hash
    {
        LinkModule::set_model_constraint_target_joint(victim, anchor);
    }
}

unsafe extern "C" fn hold_pre(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let airborne = state_entry(b).and_then(|e| STATES[e].grab).map_or_else(|| air(b), |grab| grab.airborne);
    prep(f, airborne, true)
}

unsafe extern "C" fn hold_main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    GrabModule::clear_all(b);
    KineticModule::clear_speed_all(b);
    if air(b) {
        KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
        KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_CONTROL);
        KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_MOTION);
    }
    f.sub_shift_status_main(L2CValue::Ptr(hold_loop as *const () as _))
}

unsafe extern "C" fn hold_loop(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = state_entry(b) else { return recover(b, air(b)) };
    let Some(grab) = STATES[e].grab else { return recover(b, air(b)) };
    if grab.airborne != air(b) {
        return recover(b, grab.airborne);
    }
    if !grab.holding {
        let target = LinkModule::get_node_object_id(b, *LINK_NO_CAPTURE) as u32;
        STATES[e].grab.as_mut().unwrap().target = target;
        if let Some(victim) = linked_victim(b, target) {
            if StatusModule::status_kind(victim) == *FIGHTER_STATUS_KIND_CLUNG_CAPTAIN {
                space_victim(victim);
                STATES[e].grab.as_mut().unwrap().holding = true;
                motion(b, if grab.airborne { "slayer_airbite_exe" } else { "slayer_bu_exe" });
                skyline::println!("[Slayer Down B] linked owner={} target={}", (*b).battle_object_id, target);
                return 0.into();
            }
        }
        if grab.wait_ticks >= 3 {
            skyline::println!("[Slayer Down B] capture handshake expired target={}", target);
            return recover(b, grab.airborne);
        }
        if !StopModule::is_stop(b) {
            STATES[e].grab.as_mut().unwrap().wait_ticks += 1;
        }
        return 0.into();
    }
    if !grab.released {
        let Some(victim) = linked_victim(b, grab.target) else { return recover(b, grab.airborne) };
        space_victim(victim);
    }
    let frame = MotionModule::frame(b);
    let lr = PostureModule::lr(b);
    if !grab.airborne && frame >= 20.0 && !grab.stepped {
        STATES[e].grab.as_mut().unwrap().stepped = true;
        PostureModule::add_pos(b, &Vector3f { x: 1.5 * lr, y: 0.0, z: 0.0 });
    }
    if grab.airborne && grab.released {
        if !grab.bounced {
            STATES[e].grab.as_mut().unwrap().bounced = true;
            air_recoil(f, lr);
        } else if !StopModule::is_stop(b) {
            let x = KineticModule::get_sum_speed_x(b, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN) * 0.95;
            sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_STOP, x, 0.0);
        }
    }
    if frame >= if grab.airborne { 54.0 } else { 84.0 } {
        return recover(b, grab.airborne);
    }
    0.into()
}

unsafe fn end_capture(b: *mut app::BattleObjectModuleAccessor, cut: bool) {
    let Some(e) = state_entry(b) else { return };
    let Some(grab) = STATES[e].grab.take() else { return };
    if cut && linked_victim(b, grab.target).is_some() {
        CatchModule::set_send_cut_event(b, true);
        CatchModule::cling_cut(b, false);
    }
    ACTIVE_GRABBERS[e].store(NO_TARGET, Ordering::Release);
    if grab.target != NO_TARGET {
        skyline::println!("[Slayer Down B] exit owner={} target={} released={}", (*b).battle_object_id, grab.target, grab.released);
    }
}

unsafe extern "C" fn hold_end(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let native_cut = StatusModule::status_kind_next(b) == *FIGHTER_STATUS_KIND_CATCH_CUT;
    end_capture(b, !native_cut);
    0.into()
}

unsafe fn air_recoil(f: &mut L2CFighterCommon, lr: f32) {
    let b = f.module_accessor;
    KineticModule::change_kinetic(b, *FIGHTER_KINETIC_TYPE_AIR_STOP);
    KineticModule::clear_speed_all(b);
    KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_MOTION);
    KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_CONTROL);
    sv_kinetic_energy!(reset_energy, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, ENERGY_GRAVITY_RESET_TYPE_GRAVITY, 0.0, 0.75, 0.0, 0.0, 0.0);
    sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 0.75);
    sv_kinetic_energy!(set_accel, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, -0.075);
    sv_kinetic_energy!(set_stable_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 10.0);
    sv_kinetic_energy!(set_limit_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 10.0);
    KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
    sv_kinetic_energy!(reset_energy, f, FIGHTER_KINETIC_ENERGY_ID_STOP, ENERGY_STOP_RESET_TYPE_AIR, -0.5 * lr, 0.0, 0.0, 0.0, 0.0);
    sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_STOP, -0.5 * lr, 0.0);
    sv_kinetic_energy!(set_brake, f, FIGHTER_KINETIC_ENERGY_ID_STOP, 0.0, 0.0);
    sv_kinetic_energy!(set_accel, f, FIGHTER_KINETIC_ENERGY_ID_STOP, 0.0, 0.0);
    sv_kinetic_energy!(set_limit_speed, f, FIGHTER_KINETIC_ENERGY_ID_STOP, -1.0, -1.0);
    KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_STOP);
}

// Falcon's special capture registers an absolute CATCH attack alongside its grab volume.
unsafe fn arm_catch(a: &mut L2CAgentBase) {
    macros::ATTACK_ABS(a, *FIGHTER_ATTACK_ABSOLUTE_KIND_CATCH, 0, 3.0, 0, 50, 0, 70, 0.0, 1.0,
        *ATTACK_LR_CHECK_F, 0.0, true, Hash40::new("collision_attr_normal"),
        *ATTACK_SOUND_LEVEL_S, *COLLISION_SOUND_ATTR_NONE, *ATTACK_REGION_THROW);
}

// Grab attempts. Source sly408_02 attack box: z 4.55-12.65, y 10.5-18 (active frames 7-8). GGST throws ignore
// height, so the capsule extends down to the floor to reach shorter Smash fighters (adaptation).
unsafe extern "C" fn bu(a: &mut L2CAgentBase) {
    smash::app::sv_animcmd::frame(a.lua_state_agent, 6.0);
    if macros::is_excute(a) {
        GrabModule::set_rebound(a.module_accessor, false);
        arm_catch(a);
        macros::CATCH(a, 0, Hash40::new("top"), 4.25, 0.0, 6.5, 3.75, Some(0.0), Some(10.0), Some(11.0), *FIGHTER_STATUS_KIND_CLUNG_CAPTAIN, *COLLISION_SITUATION_MASK_G);
        log_catch_volume(a);
    }
    smash::app::sv_animcmd::frame(a.lua_state_agent, 8.0);
    if macros::is_excute(a) {
        GrabModule::clear_all(a.module_accessor);
    }
}
// NmlAtkAirThrow: ReAttack on the second cell (frame 2), AttackOff at 321_01 (frame 5).
// The low capsule also lets a short-hop attempt catch a standing Smash fighter.
unsafe extern "C" fn airbite(a: &mut L2CAgentBase) {
    smash::app::sv_animcmd::frame(a.lua_state_agent, 2.0);
    if macros::is_excute(a) {
        GrabModule::set_rebound(a.module_accessor, false);
        arm_catch(a);
        macros::CATCH(a, 0, Hash40::new("top"), 5.0, 0.0, 4.0, 3.0, Some(0.0), Some(11.5), Some(11.5), *FIGHTER_STATUS_KIND_CLUNG_CAPTAIN, *COLLISION_SITUATION_MASK_A);
        macros::CATCH(a, 1, Hash40::new("top"), 4.5, 0.0, -1.0, 3.5, Some(0.0), Some(3.5), Some(10.5), *FIGHTER_STATUS_KIND_CLUNG_CAPTAIN, *COLLISION_SITUATION_MASK_G);
        log_catch_volume(a);
    }
    smash::app::sv_animcmd::frame(a.lua_state_agent, 5.0);
    if macros::is_excute(a) {
        GrabModule::clear_all(a.module_accessor);
    }
}

unsafe fn log_catch_volume(a: &mut L2CAgentBase) {
    let b = a.module_accessor;
    skyline::println!("[Slayer Down B] grab volume active={} status={:#x}",
        GrabModule::is_grab(b, 0), StatusModule::status_kind(b));
}

unsafe fn release(a: &mut L2CAgentBase) -> Option<f32> {
    let b = a.module_accessor;
    let e = state_entry(b)?;
    let grab = STATES[e].grab?;
    if StatusModule::status_kind(b) != *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING
        || !grab.holding || grab.released
    {
        return None;
    }
    let victim = linked_victim(b, grab.target)?;
    if StatusModule::status_kind(victim) != *FIGHTER_STATUS_KIND_CLUNG_CAPTAIN {
        return None;
    }
    let before = DamageModule::damage(victim, 0);
    let target = grab.target;
    STATES[e].grab.as_mut().unwrap().released = true;
    skyline::println!("[Slayer Down B] release owner={} target={}", (*b).battle_object_id, target);
    AttackModule::hit_absolute_joint(
        b,
        *FIGHTER_ATTACK_ABSOLUTE_KIND_THROW,
        target,
        Hash40::new("throw"),
        0,
        0,
    );
    let dealt = DamageModule::damage(victim, 0) - before;
    Some(if dealt > 0.0 { dealt } else { bite_damage(b) })
}
unsafe fn bite_damage(b: *mut app::BattleObjectModuleAccessor) -> f32 {
    BITE_DAMAGE * if universe_armed(b) { UNIVERSE_POWER } else { 1.0 }
}

// sly_SP_09_exe: AtkDamage 80 (x 2/11 = 14.5), vector 2000/17000 (83 deg), AtkDrain 20, bite at cell 409_11 (frame 45).
unsafe extern "C" fn bu_exe(a: &mut L2CAgentBase) {
    if macros::is_excute(a) {
        macros::ATTACK_ABS(a, *FIGHTER_ATTACK_ABSOLUTE_KIND_THROW, 0, bite_damage(a.module_accessor), 83, 70, 0, 60, 0.0, 1.0, *ATTACK_LR_CHECK_F, 0.0, true, Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_NONE, *ATTACK_REGION_THROW);
        macros::ATTACK_ABS(a, *FIGHTER_ATTACK_ABSOLUTE_KIND_CATCH, 0, 3.0, 361, 100, 0, 60, 0.0, 1.0, *ATTACK_LR_CHECK_F, 0.0, true, Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_S, *COLLISION_SOUND_ATTR_NONE, *ATTACK_REGION_THROW);
    }
    smash::app::sv_animcmd::frame(a.lua_state_agent, 45.0);
    if macros::is_excute(a) {
        if let Some(dealt) = release(a) {
            DamageModule::heal(a.module_accessor, -0.5 * dealt, 0);
            if let Some(e) = state_entry(a.module_accessor) {
                STATES[e].buff = true;
                STATES[e].buff_age = 0;
                STATES[e].buff_end_age = None;
            }
            skyline::println!("[Slayer Down B] healed {:.2}; Bloodsucking Universe armed", 0.5 * dealt);
        }
    }
}
// AirThrowExe: AtkLevel 4, AtkDamage 80 (14.5), vector 15000/10000 (34 deg), release at cell 320_12 (frame 35).
unsafe extern "C" fn airbite_exe(a: &mut L2CAgentBase) {
    if macros::is_excute(a) {
        macros::ATTACK_ABS(a, *FIGHTER_ATTACK_ABSOLUTE_KIND_THROW, 0, bite_damage(a.module_accessor), 34, 60, 0, 60, 0.0, 1.0, *ATTACK_LR_CHECK_F, 0.0, true, Hash40::new("collision_attr_cutup"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_NONE, *ATTACK_REGION_THROW);
        macros::ATTACK_ABS(a, *FIGHTER_ATTACK_ABSOLUTE_KIND_CATCH, 0, 3.0, 361, 100, 0, 60, 0.0, 1.0, *ATTACK_LR_CHECK_F, 0.0, true, Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_S, *COLLISION_SOUND_ATTR_NONE, *ATTACK_REGION_THROW);
    }
    smash::app::sv_animcmd::frame(a.lua_state_agent, 35.0);
    if macros::is_excute(a) && release(a).is_some() {
        if let Some(e) = state_entry(a.module_accessor) {
            spend_universe(&mut STATES[e]);
        }
    }
    super::shake::at(a, 36.0, super::shake::Size::M); // cmn_screenshake[3,3] at 36
}
pub(crate) unsafe fn universe_armed(b: *mut app::BattleObjectModuleAccessor) -> bool {
    entry(b).is_some_and(|e| STATES[e].object == (*b).battle_object_id && STATES[e].buff)
}

pub(crate) unsafe fn universe_phase(b: *mut app::BattleObjectModuleAccessor) -> (Option<f32>, Option<f32>) {
    let Some(e) = entry(b) else { return (None, None); };
    let state = STATES[e];
    if state.object != (*b).battle_object_id {
        return (None, None);
    }
    (state.buff.then_some(state.buff_age as f32), state.buff_end_age.map(f32::from))
}

fn spend_universe(state: &mut State) {
    if state.buff {
        state.buff = false;
        state.buff_end_age = Some(0);
    }
}

// The buff survives whiffs and shields. Only a damaging attack spends it.
unsafe extern "C" fn universe_frame(f: &mut L2CFighterCommon) {
    let b = f.module_accessor;
    let Some(e) = state_entry(b) else { return };
    let status = StatusModule::status_kind(b);
    if status == *FIGHTER_STATUS_KIND_DEAD || status == *FIGHTER_STATUS_KIND_REBIRTH
        || status == *FIGHTER_STATUS_KIND_WIN || !app::sv_information::is_ready_go()
    {
        end_capture(b, true);
        STATES[e].buff = false;
        STATES[e].buff_end_age = None;
    }
    let state = &mut STATES[e];
    let strike = [
        *FIGHTER_STATUS_KIND_ATTACK, *FIGHTER_STATUS_KIND_ATTACK_DASH,
        *FIGHTER_STATUS_KIND_ATTACK_S3, *FIGHTER_STATUS_KIND_ATTACK_HI3,
        *FIGHTER_STATUS_KIND_ATTACK_LW3, *FIGHTER_STATUS_KIND_ATTACK_S4,
        *FIGHTER_STATUS_KIND_ATTACK_HI4, *FIGHTER_STATUS_KIND_ATTACK_LW4,
        *FIGHTER_STATUS_KIND_ATTACK_AIR, *FIGHTER_STATUS_KIND_SPECIAL_N,
        *FIGHTER_STATUS_KIND_SPECIAL_S, *FIGHTER_STATUS_KIND_THROW,
        *FIGHTER_STATUS_KIND_CLIFF_ATTACK, *FIGHTER_STATUS_KIND_DOWN_STAND_ATTACK,
    ].contains(&status);
    if state.powered && strike && AttackModule::is_infliction(b, *COLLISION_KIND_MASK_HIT) {
        spend_universe(state);
        skyline::println!("[Slayer Down B] Bloodsucking Universe spent");
    }
    let powered = state.buff && strike;
    if powered || state.powered {
        AttackModule::set_power_mul_status(b, if powered { UNIVERSE_POWER } else { 1.0 });
    }
    state.powered = powered;
    if !StopModule::is_stop(b) {
        if state.buff {
            state.buff_age = state.buff_age.saturating_add(1);
        }
        state.buff_end_age = state.buff_end_age.and_then(|age| (age < 47).then_some(age + 1));
    }
}
unsafe extern "C" fn empty(_: &mut L2CAgentBase) {}
pub fn install() {
    unsafe { install_capture_depth_guard() };
    let mut agent = Agent::new("captain");
    agent
        .set_costume(crate::slots())
        .status(Pre, *FIGHTER_STATUS_KIND_SPECIAL_LW, lw_pre)
        .status(Pre, *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING, hold_pre)
        .status(Init, *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING, noop)
        .status(Main, *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING, hold_main)
        .status(Exec, *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING, noop)
        .status(End, *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING, hold_end)
        .status(Exit, *FIGHTER_CAPTAIN_STATUS_KIND_SPECIAL_HI_CLING, noop)
        .sound_acmd("sound_slayerbuexe", super::audio::bloodsucking_universe, Priority::High)
        .sound_acmd("sound_slayerairbite", super::audio::air_bite_attempt, Priority::High)
        .sound_acmd("sound_slayerairbiteexe", super::audio::air_bite_release, Priority::High)
        .game_acmd("game_slayerbu", bu, Priority::High)
        .game_acmd("game_slayerairbite", airbite, Priority::High)
        .game_acmd("game_slayerbuexe", bu_exe, Priority::High)
        .game_acmd("game_slayerairbiteexe", airbite_exe, Priority::High)
        .on_line(Main, universe_frame);
    for name in ["slayerbu", "slayerairbite", "slayerbuexe", "slayerairbiteexe"] {
        if name == "slayerbu" {
            agent.sound_acmd(format!("sound_{}", name).as_str(), empty, Priority::High);
        }
        agent
            .effect_acmd(format!("effect_{}", name).as_str(), empty, Priority::High)
            .expression_acmd(format!("expression_{}", name).as_str(), empty, Priority::High);
    }
    agent.install();
}
