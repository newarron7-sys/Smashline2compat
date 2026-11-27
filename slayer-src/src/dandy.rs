//! Source Dandy Step P/K and all four branches. Engine launch/collision use Smash rules.
use super::locomotion::{entry, facing, release_drive, settings, speed};
use smash::{
    app::{self, lua_bind::*, GroundCorrectKind, SituationKind},
    lib::{lua_const::*, L2CValue},
    lua2cpp::{L2CAgentBase, L2CFighterCommon},
    phx::{Hash40, Vector3f},
};
use smash_script::{lua_args, macros, sv_kinetic_energy};
use smashline::{Agent, Priority};
#[derive(Clone, Copy)]
struct State {
    back: bool,
    crossed: bool,
    branch_checked: bool,
    stage: u8,
    lr: f32,
    travel_lr: f32,
    carry: f32,
    buffer: u8,
    buffer_until: f32,
    frame: f32,
    vy: f32,
    airborne: bool,
    lifted: bool,
    slowed: bool,
    diving: bool,
}
impl State {
    const NEW: Self = Self {
        back: false,
        crossed: false,
        branch_checked: false,
        stage: 0,
        lr: 1.0,
        travel_lr: 1.0,
        carry: 0.0,
        buffer: 0,
        buffer_until: -1.0,
        frame: -1.0,
        vy: 0.0,
        airborne: false,
        lifted: false,
        slowed: false,
        diving: false,
    };
}
static mut STATES: [State; 8] = [State::NEW; 8];
pub unsafe extern "C" fn pre(f: &mut L2CFighterCommon) -> L2CValue {
    settings(f, false);
    0.into()
}
pub unsafe extern "C" fn main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return 0.into();
    };
    let lr = facing(b);
    STATES[e] = State {
        back: ControlModule::get_stick_x(b) * lr < -0.2,
        lr,
        travel_lr: lr,
        ..State::NEW
    };
    PostureModule::set_lr(b, lr);
    PostureModule::update_rot_y_lr(b);
    AttackModule::clear_all(b);
    motion(
        b,
        if STATES[e].back {
            "slayer_dandy_k"
        } else {
            "slayer_dandy_p"
        },
        0.0,
    );
    if StatusModule::prev_status_kind(b, 0) == *FIGHTER_STATUS_KIND_DASH {
        STATES[e].carry = 1.275 * lr;
    }
    speed(f, STATES[e].carry);
    f.sub_shift_status_main(L2CValue::Ptr(loop_main as *const () as _))
}
unsafe fn motion(b: *mut app::BattleObjectModuleAccessor, name: &str, frame: f32) {
    MotionModule::change_motion(b, Hash40::new(name), frame, 1.0, false, 0.0, false, false);
}
unsafe fn start_followup(f: &mut L2CFighterCommon, e: usize, choice: u8) {
    let b = f.module_accessor;
    let st = &mut STATES[e];
    st.carry = super::dandy_math::velocity(st.back, MotionModule::frame(b)) * st.travel_lr;
    st.stage = choice;
    st.frame = -1.0;
    st.buffer = 0;
    JostleModule::set_status(b, true);
    AttackModule::clear_all(b);
    motion(
        b,
        match choice {
            1 => "slayer_pilebunker",
            2 => "slayer_bumpahead",
            3 => "slayer_itslate",
            _ => "slayer_hammer_air",
        },
        0.0,
    );

}
unsafe extern "C" fn loop_main(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    let Some(e) = entry(b) else {
        return 0.into();
    };
    let fr = MotionModule::frame(b);
    if STATES[e].stage != 4 && StatusModule::situation_kind(b) != *SITUATION_KIND_GROUND {
        f.change_status((*FIGHTER_STATUS_KIND_FALL).into(), false.into());
        return 1.into();
    }
    if StopModule::is_stop(b) {
        return 0.into();
    }
    let fresh = fr != STATES[e].frame;
    STATES[e].frame = fr;
    if STATES[e].stage == 0 {
        // P checks for a cross-through after its first ten frames, then turns once.
        if !STATES[e].back && !STATES[e].branch_checked && fr >= 10.0 {
            STATES[e].branch_checked = true;
            let target = facing(b);
            if target * STATES[e].lr < 0.0 {
                STATES[e].crossed = true;
                STATES[e].lr = target;
                PostureModule::set_lr(b, target);
                PostureModule::update_rot_y_lr(b);
                motion(b, "slayer_dandy_cross", fr);
            }
        }
        if fresh
            && (ControlModule::check_button_trigger(b, *CONTROL_PAD_BUTTON_ATTACK)
                || ControlModule::check_button_trigger(b, *CONTROL_PAD_BUTTON_SPECIAL))
        {
            STATES[e].buffer = super::dandy_math::followup(
                ControlModule::get_stick_x(b),
                ControlModule::get_stick_y(b),
                STATES[e].lr,
            );
            STATES[e].buffer_until = fr + 5.0;
        }
        if STATES[e].buffer != 0 && fr > STATES[e].buffer_until {
            STATES[e].buffer = 0;
        }
        if super::dandy_math::window(STATES[e].back, fr) && STATES[e].buffer != 0 {
            let choice = STATES[e].buffer;
            start_followup(f, e, choice);
            return 0.into();
        }
        let free = if STATES[e].back {
            fr >= 2.0 && fr < 9.0
        } else {
            fr >= 4.0 && fr < 9.0
        };
        JostleModule::set_status(b, !free);
        speed(
            f,
            super::dandy_math::velocity(STATES[e].back, fr) * STATES[e].travel_lr
                + STATES[e].carry * 0.85_f32.powi(fr.floor() as i32),
        );
        if fr >= if STATES[e].back { 40.0 } else { 34.0 } {
            f.change_status((*FIGHTER_STATUS_KIND_WAIT).into(), false.into());
            return 1.into();
        }
    } else if STATES[e].stage == 4 {
        // Master's Hammer, source sly_SP_08 (0-based motion frames, 20000 script units per Smash unit):
        // f2 SetSpeedX 10000 / SetSpeedY 40000 / gravity 1200; f4 AddPosY 50000; f7 PerSpeedY 30;
        // f13 PerSpeedXEveryFrame 80; f19 SetSpeedY -40000 / gravity 10000; landing goes to the hit.
        // Vertical motion uses the GRAVITY energy: the STOP energy's AIR reset only carries X, so v0.3.17 floated.
        if fr >= 2.0 && !STATES[e].airborne {
            STATES[e].airborne = true;
            StatusModule::set_situation_kind(b, SituationKind(*SITUATION_KIND_AIR), true);
            GroundModule::correct(b, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
            KineticModule::change_kinetic(b, *FIGHTER_KINETIC_TYPE_AIR_STOP);
            KineticModule::clear_speed_all(b);
            KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_MOTION);
            KineticModule::unable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_CONTROL);
            sv_kinetic_energy!(reset_energy, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, ENERGY_GRAVITY_RESET_TYPE_GRAVITY, 0.0, 2.0, 0.0, 0.0, 0.0);
            sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 2.0);
            sv_kinetic_energy!(set_accel, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, -0.06);
            sv_kinetic_energy!(set_stable_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 10.0);
            sv_kinetic_energy!(set_limit_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, 10.0);
            KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
            sv_kinetic_energy!(reset_energy, f, FIGHTER_KINETIC_ENERGY_ID_STOP, ENERGY_STOP_RESET_TYPE_AIR, 0.0, 0.0, 0.0, 0.0, 0.0);
            sv_kinetic_energy!(set_brake, f, FIGHTER_KINETIC_ENERGY_ID_STOP, 0.0, 0.0);
            sv_kinetic_energy!(set_accel, f, FIGHTER_KINETIC_ENERGY_ID_STOP, 0.0, 0.0);
            sv_kinetic_energy!(set_limit_speed, f, FIGHTER_KINETIC_ENERGY_ID_STOP, -1.0, -1.0);
            KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_STOP);
        }
        if fr >= 4.0 && !STATES[e].lifted {
            STATES[e].lifted = true;
            PostureModule::add_pos(
                b,
                &Vector3f {
                    x: 0.0,
                    y: 2.5,
                    z: 0.0,
                },
            );
        }
        if fr >= 7.0 && !STATES[e].slowed {
            STATES[e].slowed = true;
            let vy = KineticModule::get_sum_speed_y(b, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN);
            sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, vy * 0.3);
        }
        if fr >= 19.0 && !STATES[e].diving {
            STATES[e].diving = true;
            sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, -2.0);
            sv_kinetic_energy!(set_accel, f, FIGHTER_KINETIC_ENERGY_ID_GRAVITY, -0.5);
        }
        let falling = STATES[e].slowed
            && KineticModule::get_sum_speed_y(b, *KINETIC_ENERGY_RESERVE_ATTRIBUTE_MAIN) <= 0.0;
        if STATES[e].airborne
            && fr > 4.0
            && (StatusModule::situation_kind(b) == *SITUATION_KIND_GROUND
                || (falling && GroundModule::is_touch(b, *GROUND_TOUCH_FLAG_DOWN as u32)))
        {
            STATES[e].stage = 5;
            STATES[e].frame = -1.0;
            StatusModule::set_situation_kind(b, SituationKind(*SITUATION_KIND_GROUND), true);
            GroundModule::correct(b, GroundCorrectKind(*GROUND_CORRECT_KIND_GROUND_CLIFF_STOP));
            KineticModule::clear_speed_all(b);
            speed(f, 0.0);
            motion(b, "slayer_hammer_land", 0.0);
            return 0.into();
        }
        let x = if fr < 2.0 {
            STATES[e].carry * 0.25
        } else {
            0.5 * STATES[e].lr
                * if fr >= 13.0 {
                    0.8_f32.powi(fr.floor() as i32 - 13)
                } else {
                    1.0
                }
        };
        if STATES[e].airborne {
            sv_kinetic_energy!(set_speed, f, FIGHTER_KINETIC_ENERGY_ID_STOP, x, 0.0);
        } else {
            speed(f, x);
        }
        if fr >= 82.0 {
            f.change_status((*FIGHTER_STATUS_KIND_FALL).into(), false.into());
            return 1.into();
        }
    } else {
        let st = STATES[e];
        let x = if st.stage == 1 {
            let initial = st.carry * 0.5 + 1.75 * st.lr;
            initial
                * if fr < 4.0 {
                    0.95_f32.powi(fr.floor() as i32)
                } else {
                    0.95_f32.powi(3) * 0.9_f32.powi(fr.floor() as i32 - 3)
                }
        } else if st.stage == 5 {
            0.0
        } else {
            st.carry * 0.85_f32.powi(fr.floor() as i32)
        };
        speed(f, x);
        let cancel = match st.stage {
            1 => 43.0,
            2 => 31.0,
            5 => 27.0,
            _ => 1000.0,
        };
        if fr >= cancel {
            CancelModule::enable_cancel(b);
            if f.sub_wait_ground_check_common(false.into()).get_bool() {
                return 1.into();
            }
        }
        let end = match st.stage {
            1 => 48.0,
            2 => 55.0,
            3 => 58.0,
            _ => 31.0,
        };
        if fr >= end {
            f.change_status((*FIGHTER_STATUS_KIND_WAIT).into(), false.into());
            return 1.into();
        }
    }
    0.into()
}
pub unsafe extern "C" fn end(f: &mut L2CFighterCommon) -> L2CValue {
    let b = f.module_accessor;
    AttackModule::clear_all(b);
    JostleModule::set_status(b, true);
    release_drive(b);
    if StatusModule::situation_kind(b) == *SITUATION_KIND_AIR {
        KineticModule::enable_energy(b, *FIGHTER_KINETIC_ENERGY_ID_GRAVITY);
    }
    0.into()
}
unsafe fn damage(a: &mut L2CAgentBase, source: f32) -> f32 {
    let factor = if let Some(e) = entry(a.module_accessor) {
        if STATES[e].back {
            1.0
        } else {
            0.7
        }
    } else {
        1.0
    };
    source * (2.0 / 11.0) * factor
}
// Source capsules, follow-up scripts and registration are generated from JONB below.
#[derive(Clone, Copy)]
struct Bounce {
    victim: u32,
    frames: i32,
    apply: bool,
    x: f32,
    y: f32,
}
static mut BOUNCES: [Bounce; 8] = [Bounce {
    victim: u32::MAX,
    frames: 0,
    apply: false,
    x: 0.0,
    y: 0.0,
}; 8];
#[derive(Clone, Copy)]
struct Block {
    victim: u32,
    frames: i32,
    stiff: i32,
}
static mut BLOCKS: [Block; 8] = [Block {
    victim: u32::MAX,
    frames: 0,
    stiff: 0,
}; 8];
pub(crate) unsafe fn collision(
    a: *mut app::BattleObjectModuleAccessor,
    d: *mut app::BattleObjectModuleAccessor,
) {
    let Some(e) = entry(a) else {
        return;
    };
    if StatusModule::status_kind(a) != *FIGHTER_STATUS_KIND_SPECIAL_S {
        return;
    }
    let stage = STATES[e].stage;
    let status = StatusModule::status_kind(d);
    if [
        *FIGHTER_STATUS_KIND_GUARD_ON,
        *FIGHTER_STATUS_KIND_GUARD,
        *FIGHTER_STATUS_KIND_GUARD_DAMAGE,
    ]
    .contains(&status)
        || AttackModule::is_infliction(a, *COLLISION_KIND_MASK_SHIELD)
    {
        // Guard crush uses the source forced block-recovery lengths, not shield depletion.
        let stiff = match stage {
            1 => 25,
            3 => 36,
            5 => 53,
            _ => 0,
        };
        if stiff > 0 {
            BLOCKS[e] = Block {
                victim: (*d).battle_object_id,
                frames: 8,
                stiff,
            };
        }
    } else if [3, 5].contains(&stage)
        && StatusModule::situation_kind(d) == *SITUATION_KIND_AIR
        && AttackModule::is_infliction(a, *COLLISION_KIND_MASK_HIT)
    {
        BOUNCES[e] = Bounce {
            victim: (*d).battle_object_id,
            frames: 120,
            apply: false,
            x: if stage == 3 { 0.85 } else { 0.5 } * STATES[e].lr,
            y: if stage == 3 { 1.6 } else { 1.4 },
        };
    }
}
unsafe extern "C" fn victim_frame(f: &mut L2CFighterCommon) {
    let Some(e) = entry(f.module_accessor) else {
        return;
    };
    let block = &mut BLOCKS[e];
    if block.frames > 0 && app::sv_battle_object::is_active(block.victim) {
        let d = app::sv_battle_object::module_accessor(block.victim);
        if !d.is_null() {
            if StatusModule::status_kind(d) == *FIGHTER_STATUS_KIND_GUARD_DAMAGE {
                if !WorkModule::is_flag(d, *FIGHTER_STATUS_GUARD_ON_WORK_FLAG_JUST_SHIELD) {
                    WorkModule::set_int(
                        d,
                        block.stiff.max(WorkModule::get_int(
                            d,
                            *FIGHTER_STATUS_GUARD_DAMAGE_WORK_INT_STIFF_FRAME,
                        )),
                        *FIGHTER_STATUS_GUARD_DAMAGE_WORK_INT_STIFF_FRAME,
                    );
                }
                block.frames = 0;
            } else if !StopModule::is_stop(d) {
                block.frames -= 1;
            }
        } else {
            block.frames = 0;
        }
    }
    let record = &mut BOUNCES[e];
    if record.frames <= 0 || !app::sv_battle_object::is_active(record.victim) {
        record.frames = 0;
        return;
    }
    let d = app::sv_battle_object::module_accessor(record.victim);
    if d.is_null() {
        record.frames = 0;
        return;
    }
    if StopModule::is_stop(d) {
        return;
    }
    record.frames -= 1;
    let status = StatusModule::status_kind(d);
    if record.apply {
        if status == *FIGHTER_STATUS_KIND_DAMAGE_FLY {
            KineticModule::clear_speed_all(d);
            KineticModule::add_speed(
                d,
                &Vector3f {
                    x: record.x,
                    y: record.y,
                    z: 0.0,
                },
            );
            record.frames = 0;
        }
        return;
    }
    if ![
        *FIGHTER_STATUS_KIND_DAMAGE_AIR,
        *FIGHTER_STATUS_KIND_DAMAGE_FLY,
        *FIGHTER_STATUS_KIND_DAMAGE_FLY_METEOR,
        *FIGHTER_STATUS_KIND_DOWN,
        *FIGHTER_STATUS_KIND_DOWN_WAIT,
        *FIGHTER_STATUS_KIND_DAMAGE,
    ]
    .contains(&status)
    {
        record.frames = 0;
        return;
    }
    if StatusModule::situation_kind(d) == *SITUATION_KIND_GROUND {
        record.apply = true;
        StatusModule::set_situation_kind(d, SituationKind(*SITUATION_KIND_AIR), true);
        GroundModule::correct(d, GroundCorrectKind(*GROUND_CORRECT_KIND_AIR));
        PostureModule::add_pos(
            d,
            &Vector3f {
                x: 0.0,
                y: 0.1,
                z: 0.0,
            },
        );
        StatusModule::change_status_request_from_script(d, *FIGHTER_STATUS_KIND_DAMAGE_FLY, false);
    }
}

// Source sly_SP_05: attack cells on frames 5-7 (Dustloop: Pilebunker 17+5, 3 active).
unsafe extern "C" fn pile(a: &mut L2CAgentBase) {
    smash::app::sv_animcmd::frame(a.lua_state_agent, 5.0); super::shake::now(a, super::shake::Size::M); // Impact 800 at 5
    super::native_fx::now(a, "slayer_stone_burst", false); // SLY_400_stoneburst at 5
    if macros::is_excute(a) {
        let damage = damage(a, 80.0);
        macros::ATTACK(
            a,
            0,
            0,
            Hash40::new("top"),
            damage,
            20,
            85,
            0,
            45,
            5.5,
            0.0,
            12.750000,
            3.750000,
            Some(0.0),
            Some(12.750000),
            Some(15.0),
            super::hitstop::multiplier(damage, 24.0),
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
        macros::ATTACK(
            a,
            1,
            0,
            Hash40::new("top"),
            damage,
            20,
            85,
            0,
            45,
            5.0,
            0.0,
            6.000000,
            3.500000,
            Some(0.0),
            Some(6.000000),
            Some(9.0),
            super::hitstop::multiplier(damage, 24.0),
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
    }
    smash::app::sv_animcmd::frame(a.lua_state_agent, 8.0);
    if macros::is_excute(a) {
        AttackModule::clear_all(a.module_accessor);
    }
}

unsafe extern "C" fn bump(a: &mut L2CAgentBase) {
    smash::app::sv_animcmd::frame(a.lua_state_agent, 13.0); super::shake::now(a, super::shake::Size::S); // cmn_screenshake[1,2] at 13
    if macros::is_excute(a) {
        let damage = damage(a, 40.0);
        macros::ATTACK(
            a,
            0,
            0,
            Hash40::new("top"),
            damage,
            65,
            45,
            0,
            35,
            3.0,
            0.0,
            2.200000,
            5.500000,
            Some(0.0),
            Some(2.200000),
            Some(20.0),
            super::hitstop::multiplier(damage, 18.0),
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
    }
    smash::app::sv_animcmd::frame(a.lua_state_agent, 16.0);
    if macros::is_excute(a) {
        AttackModule::clear_all(a.module_accessor);
    }
}

unsafe extern "C" fn late(a: &mut L2CAgentBase) {
    smash::app::sv_animcmd::frame(a.lua_state_agent, 21.0); super::shake::now(a, super::shake::Size::L); // Vertical 3000 at 21
    if macros::is_excute(a) {
        let damage = damage(a, 50.0);
        macros::ATTACK(
            a,
            0,
            0,
            Hash40::new("top"),
            damage,
            80,
            65,
            0,
            45,
            5.5,
            0.0,
            4.500000,
            8.500000,
            Some(0.0),
            Some(16.500000),
            Some(13.0),
            super::hitstop::multiplier(damage, 20.0),
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
            *COLLISION_SITUATION_MASK_G,
            *COLLISION_CATEGORY_MASK_ALL,
            *COLLISION_PART_MASK_ALL,
            false,
            Hash40::new("collision_attr_normal"),
            *ATTACK_SOUND_LEVEL_L,
            *COLLISION_SOUND_ATTR_PUNCH,
            *ATTACK_REGION_PUNCH,
        );
        macros::ATTACK(
            a,
            1,
            0,
            Hash40::new("top"),
            damage,
            270,
            65,
            0,
            45,
            5.5,
            0.0,
            4.500000,
            8.500000,
            Some(0.0),
            Some(16.500000),
            Some(13.0),
            super::hitstop::multiplier(damage, 20.0),
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
            *COLLISION_SITUATION_MASK_A,
            *COLLISION_CATEGORY_MASK_ALL,
            *COLLISION_PART_MASK_ALL,
            false,
            Hash40::new("collision_attr_normal"),
            *ATTACK_SOUND_LEVEL_L,
            *COLLISION_SOUND_ATTR_PUNCH,
            *ATTACK_REGION_PUNCH,
        );
        macros::ATTACK(
            a,
            2,
            0,
            Hash40::new("top"),
            damage,
            80,
            65,
            0,
            45,
            5.0,
            0.0,
            24.000000,
            6.000000,
            Some(0.0),
            Some(26.000000),
            Some(6.000000),
            super::hitstop::multiplier(damage, 20.0),
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
            *COLLISION_SITUATION_MASK_G,
            *COLLISION_CATEGORY_MASK_ALL,
            *COLLISION_PART_MASK_ALL,
            false,
            Hash40::new("collision_attr_normal"),
            *ATTACK_SOUND_LEVEL_L,
            *COLLISION_SOUND_ATTR_PUNCH,
            *ATTACK_REGION_PUNCH,
        );
        macros::ATTACK(
            a,
            3,
            0,
            Hash40::new("top"),
            damage,
            270,
            65,
            0,
            45,
            5.0,
            0.0,
            24.000000,
            6.000000,
            Some(0.0),
            Some(26.000000),
            Some(6.000000),
            super::hitstop::multiplier(damage, 20.0),
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
            *COLLISION_SITUATION_MASK_A,
            *COLLISION_CATEGORY_MASK_ALL,
            *COLLISION_PART_MASK_ALL,
            false,
            Hash40::new("collision_attr_normal"),
            *ATTACK_SOUND_LEVEL_L,
            *COLLISION_SOUND_ATTR_PUNCH,
            *ATTACK_REGION_PUNCH,
        );
    }
    smash::app::sv_animcmd::frame(a.lua_state_agent, 24.0);
    if macros::is_excute(a) {
        AttackModule::clear_all(a.module_accessor);
    }
}

unsafe extern "C" fn hammer(a: &mut L2CAgentBase) {
    super::shake::now(a, super::shake::Size::L); // landing: Vertical 2000 at source 83
    if macros::is_excute(a) {
        let damage = damage(a, 60.0);
        macros::ATTACK(
            a,
            0,
            0,
            Hash40::new("top"),
            damage,
            80,
            70,
            0,
            50,
            5.0,
            0.0,
            4.050000,
            -5.250000,
            Some(0.0),
            Some(4.050000),
            Some(13.0),
            super::hitstop::multiplier(damage, 20.0),
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
            *COLLISION_SITUATION_MASK_G,
            *COLLISION_CATEGORY_MASK_ALL,
            *COLLISION_PART_MASK_ALL,
            false,
            Hash40::new("collision_attr_normal"),
            *ATTACK_SOUND_LEVEL_L,
            *COLLISION_SOUND_ATTR_PUNCH,
            *ATTACK_REGION_PUNCH,
        );
        macros::ATTACK(
            a,
            1,
            0,
            Hash40::new("top"),
            damage,
            270,
            70,
            0,
            50,
            5.0,
            0.0,
            4.050000,
            -5.250000,
            Some(0.0),
            Some(4.050000),
            Some(13.0),
            super::hitstop::multiplier(damage, 20.0),
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
            *COLLISION_SITUATION_MASK_A,
            *COLLISION_CATEGORY_MASK_ALL,
            *COLLISION_PART_MASK_ALL,
            false,
            Hash40::new("collision_attr_normal"),
            *ATTACK_SOUND_LEVEL_L,
            *COLLISION_SOUND_ATTR_PUNCH,
            *ATTACK_REGION_PUNCH,
        );
    }
    smash::app::sv_animcmd::frame(a.lua_state_agent, 3.0);
    if macros::is_excute(a) {
        AttackModule::clear_all(a.module_accessor);
    }
}

unsafe extern "C" fn empty(_: &mut L2CAgentBase) {}
// Dandy P/K (sly_SP_03/04): SLY_dashstep_cancel at 1 when entered from a dash.
unsafe extern "C" fn dandy_start(a: &mut L2CAgentBase) { super::native_fx::dashstep(a); }
pub fn install() {
    Agent::new("captain")
        .set_costume(crate::slots())
        .game_acmd("game_slayerdandyp", dandy_start, Priority::High)
        .sound_acmd("sound_slayerdandyp", super::audio::dandy_forward, Priority::High)
        .expression_acmd("expression_slayerdandyp", empty, Priority::High)
        .game_acmd("game_slayerdandycross", empty, Priority::High)
        .sound_acmd("sound_slayerdandycross", empty, Priority::High)
        .expression_acmd("expression_slayerdandycross", empty, Priority::High)
        .game_acmd("game_slayerdandyk", dandy_start, Priority::High)
        .sound_acmd("sound_slayerdandyk", super::audio::dandy_backward, Priority::High)
        .expression_acmd("expression_slayerdandyk", empty, Priority::High)
        .game_acmd("game_slayerpilebunker", pile, Priority::High)
        .sound_acmd("sound_slayerpilebunker", super::audio::pile, Priority::High)
        .expression_acmd("expression_slayerpilebunker", empty, Priority::High)
        .game_acmd("game_slayerbumpahead", bump, Priority::High)
        .sound_acmd("sound_slayerbumpahead", super::audio::bump, Priority::High)
        .expression_acmd("expression_slayerbumpahead", empty, Priority::High)
        .game_acmd("game_slayeritslate", late, Priority::High)
        .sound_acmd("sound_slayeritslate", super::audio::late, Priority::High)
        .expression_acmd("expression_slayeritslate", empty, Priority::High)
        .game_acmd("game_slayerhammerair", empty, Priority::High)
        .sound_acmd("sound_slayerhammerair", super::audio::hammer, Priority::High)
        .expression_acmd("expression_slayerhammerair", empty, Priority::High)
        .game_acmd("game_slayerhammerland", hammer, Priority::High)
        .sound_acmd("sound_slayerhammerland", empty, Priority::High)
        .expression_acmd("expression_slayerhammerland", empty, Priority::High)
        .on_line(smashline::Main, victim_frame)
        .install();
}
