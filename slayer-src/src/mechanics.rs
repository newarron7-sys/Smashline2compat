//! Slayer slot (c180-c196) bridge to the private copy of the user's Potemkin binary.
//! RC input/execution and turn status bodies remain original machine code.
use skyline::hooks::InlineCtx;
use smash::{
    app::{self, FighterEntryID, GroundCliffCheckKind, HitStatus, SituationKind, lua_bind::*},
    lib::{L2CValue, lua_const::*},
    lua2cpp::{L2CAgentBase, L2CFighterCommon},
    phx::{Hash40, Vector3f},
};
use smashline::{Agent, End, Main, Pre, Priority};
use std::sync::atomic::{AtomicUsize, Ordering};
static CORE: AtomicUsize = AtomicUsize::new(0);
#[skyline::hook(offset = 0x4b638, inline)]
unsafe fn original_tension_hud(ctx: &InlineCtx) {
    let base = CORE.load(Ordering::Relaxed);
    if base != 0 {
        // Correctif : la table « ce joueur a une jauge de Tension » (base + 0x579e8) n'était jamais remise à zéro.
        // Après un match avec Slayer, le joueur gardait la jauge avec n'importe quel autre perso. On la recalcule
        // ici à chaque image : seuls les Slayer présents dans le match (hors écran des résultats) l'ont.
        let compact: unsafe extern "C" fn(i64) -> usize = std::mem::transmute(base + 0xa6e4);
        let resultats = FighterManager::is_result_mode(singletons::FighterManager());
        let mut a_jauge = [0u8; 8];
        for entree in 0..8 {
            let id = app::Fighter::get_id_from_entry_id(entree);
            if id == *BATTLE_OBJECT_ID_INVALID as u32 || !app::sv_battle_object::is_active(id) {
                continue;
            }
            let b = app::sv_battle_object::module_accessor(id);
            if !resultats && slayer(b) {
                let i = compact(entree as i64);
                if i < 8 {
                    a_jauge[i] = 1;
                }
            }
        }
        for i in 0..8 {
            *((base + 0x579e8 + i) as *mut u8) = a_jauge[i];
        }
        let draw: unsafe extern "C" fn(&InlineCtx) = std::mem::transmute(base + 0x1b29c);
        draw(ctx);
    }
}
const RC: i32 = 0x1fc;
unsafe extern "C" {
    #[link_name = "_ZN3app19FighterCutInManager13is_one_on_oneEv"]
    fn is_one_on_one() -> bool;
}
#[derive(Clone, Copy)]
struct State {
    object: u32,
    ints: [i32; 7],
    floats: [f32; 0x72],
    flags: [bool; 0x1b],
    started: bool,
    previous_status: i32,
    last_damage: f32,
    last_hitbox: i32,
}
impl State {
    const NEW: Self = Self {
        object: u32::MAX,
        ints: [0; 7],
        floats: [0.0; 0x72],
        flags: [false; 0x1b],
        started: false,
        previous_status: -1,
        last_damage: 0.0,
        last_hitbox: 0,
    };
}
static mut STATES: [State; 8] = [State::NEW; 8];
unsafe fn slayer(b: *mut app::BattleObjectModuleAccessor) -> bool {
    !b.is_null()
        && app::utility::get_category(&mut *b) == *BATTLE_OBJECT_CATEGORY_FIGHTER
        && app::utility::get_kind(&mut *b) == *FIGHTER_KIND_CAPTAIN
        && crate::is_slot(WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR))
}
unsafe fn state(b: *mut app::BattleObjectModuleAccessor) -> Option<&'static mut State> {
    let i = WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID);
    if !(0..8).contains(&i) {
        return None;
    }
    let st = &mut *std::ptr::addr_of_mut!(STATES)
        .cast::<State>()
        .add(i as usize);
    if st.object != (*b).battle_object_id {
        *st = State::NEW;
        st.object = (*b).battle_object_id;
    }
    Some(st)
}
fn custom_float(id: i32) -> bool {
    (0x4c..=0x71).contains(&id)
}
fn custom_int(id: i32) -> bool {
    (0x100000c6..=0x100000cc).contains(&id)
}
fn custom_flag(id: i32) -> bool {
    (0x200000fa..=0x20000114).contains(&id)
}
#[skyline::hook(replace=WorkModule::get_float)]
unsafe fn get_float(b: *mut app::BattleObjectModuleAccessor, id: i32) -> f32 {
    if custom_float(id) && slayer(b) {
        if let Some(s) = state(b) {
            return s.floats[id as usize];
        }
    }
    original!()(b, id)
}
#[skyline::hook(replace=WorkModule::set_float)]
unsafe fn set_float(b: *mut app::BattleObjectModuleAccessor, v: f32, id: i32) {
    if custom_float(id) && slayer(b) {
        if let Some(s) = state(b) {
            s.floats[id as usize] = v;
            return;
        }
    }
    original!()(b, v, id)
}
#[skyline::hook(replace=WorkModule::get_int)]
unsafe fn get_int(b: *mut app::BattleObjectModuleAccessor, id: i32) -> i32 {
    if custom_int(id) && slayer(b) {
        if let Some(s) = state(b) {
            return s.ints[(id - 0x100000c6) as usize];
        }
    }
    original!()(b, id)
}
#[skyline::hook(replace=WorkModule::get_int64)]
unsafe fn get_int64(b: *mut app::BattleObjectModuleAccessor, id: i32) -> i64 {
    if custom_int(id) && slayer(b) {
        if let Some(s) = state(b) {
            return s.ints[(id - 0x100000c6) as usize] as i64;
        }
    }
    original!()(b, id)
}
#[skyline::hook(replace=WorkModule::set_int)]
unsafe fn set_int(b: *mut app::BattleObjectModuleAccessor, v: i32, id: i32) {
    if custom_int(id) && slayer(b) {
        if let Some(s) = state(b) {
            s.ints[(id - 0x100000c6) as usize] = v;
            return;
        }
    }
    original!()(b, v, id)
}
#[skyline::hook(replace=WorkModule::dec_int)]
unsafe fn dec_int(b: *mut app::BattleObjectModuleAccessor, id: i32) {
    if custom_int(id) && slayer(b) {
        if let Some(s) = state(b) {
            s.ints[(id - 0x100000c6) as usize] = s.ints[(id - 0x100000c6) as usize].wrapping_sub(1);
            return;
        }
    }
    original!()(b, id)
}
#[skyline::hook(replace=WorkModule::is_flag)]
unsafe fn is_flag(b: *mut app::BattleObjectModuleAccessor, id: i32) -> bool {
    if custom_flag(id) && slayer(b) {
        if let Some(s) = state(b) {
            return s.flags[(id - 0x200000fa) as usize];
        }
    }
    original!()(b, id)
}
#[skyline::hook(replace=WorkModule::on_flag)]
unsafe fn on_flag(b: *mut app::BattleObjectModuleAccessor, id: i32) {
    if custom_flag(id) && slayer(b) {
        if let Some(s) = state(b) {
            s.flags[(id - 0x200000fa) as usize] = true;
            return;
        }
    }
    original!()(b, id)
}
#[skyline::hook(replace=WorkModule::off_flag)]
unsafe fn off_flag(b: *mut app::BattleObjectModuleAccessor, id: i32) {
    if custom_flag(id) && slayer(b) {
        if let Some(s) = state(b) {
            s.flags[(id - 0x200000fa) as usize] = false;
            return;
        }
    }
    original!()(b, id)
}
#[skyline::hook(replace=WorkModule::set_flag)]
unsafe fn set_flag(b: *mut app::BattleObjectModuleAccessor, v: bool, id: i32) {
    if custom_flag(id) && slayer(b) {
        if let Some(s) = state(b) {
            s.flags[(id - 0x200000fa) as usize] = v;
            return;
        }
    }
    original!()(b, v, id)
}
unsafe fn core() -> usize {
    let cached = CORE.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let mut symbol = 0usize;
    skyline::nn::ro::LookupSymbol(&mut symbol, b"slayer_core_anchor\0".as_ptr());
    if symbol == 0 {
        return 0;
    }
    let base = symbol - 0x1b29c;
    if *(base as *const u8).add(0x1ba08).cast::<u32>() != 0xd65f03c0
        || *(base as *const u8).add(0x8874).cast::<u32>() != 0x52800020
    {
        return 0;
    }
    CORE.store(base, Ordering::Relaxed);
    skyline::println!("[Slayer] original Potemkin RC library found at {:#x}", base);
    base
}
pub(crate) unsafe fn original_status(offset: usize, f: &mut L2CFighterCommon) -> L2CValue {
    let b = core();
    if b == 0 {
        return 0.into();
    }
    let function: unsafe extern "C" fn(&mut L2CFighterCommon) -> L2CValue =
        std::mem::transmute(b + offset);
    function(f)
}
unsafe extern "C" fn rc_main(f: &mut L2CFighterCommon) -> L2CValue {
    original_status(0x17e24, f)
}
unsafe extern "C" fn rc_end(f: &mut L2CFighterCommon) -> L2CValue {
    original_status(0x179cc, f)
}
unsafe extern "C" fn ground_main(f: &mut L2CFighterCommon) -> L2CValue {
    original_status(0xe060, f)
}
unsafe extern "C" fn ground_end(f: &mut L2CFighterCommon) -> L2CValue {
    original_status(0xe020, f)
}
unsafe extern "C" fn air_main(f: &mut L2CFighterCommon) -> L2CValue {
    original_status(0xd678, f)
}
unsafe extern "C" fn air_end(f: &mut L2CFighterCommon) -> L2CValue {
    original_status(0xd638, f)
}
unsafe extern "C" fn common_pre(f: &mut L2CFighterCommon) -> L2CValue {
    StatusModule::init_settings(
        f.module_accessor,
        SituationKind(*SITUATION_KIND_NONE),
        *FIGHTER_KINETIC_TYPE_UNIQ,
        *GROUND_CORRECT_KIND_KEEP as u32,
        GroundCliffCheckKind(*GROUND_CLIFF_CHECK_KIND_NONE),
        true,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_FLAG,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_INT,
        *FIGHTER_STATUS_WORK_KEEP_FLAG_ALL_FLOAT,
        0,
    );
    0.into()
}
unsafe extern "C" fn roman_acmd(a: &mut L2CAgentBase) {
    let b = core();
    if b != 0 {
        let callback: unsafe extern "C" fn(&mut L2CAgentBase) = std::mem::transmute(b + 0x13d1c);
        let slot = WorkModule::get_int(a.module_accessor, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR);
        crate::effect_lookup::with_rc_slot(slot, || callback(a));
    }
}
unsafe fn meter_add(b: *mut app::BattleObjectModuleAccessor, amount: f32) {
    // Jauge qui se charge 2 fois plus vite (coups donnés et dégâts reçus), demandé pour suivre le rythme de Potemkin.
    let amount = if amount > 0.0 { amount * 2.0 } else { amount };
    let base = core();
    if base == 0 {
        return;
    }
    let entry = WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as i64;
    let index_fn: unsafe extern "C" fn(i64) -> usize = std::mem::transmute(base + 0xa6e4);
    let slot = index_fn(entry);
    if slot >= 8 {
        return;
    }
    let add: unsafe extern "C" fn(usize, f32) = std::mem::transmute(base + 0x67c0);
    add(slot, amount);
}
unsafe fn charged_slot(b: *mut app::BattleObjectModuleAccessor) -> Option<(usize, usize)> {
    let base = core();
    if base == 0 {
        return None;
    }
    let index: unsafe extern "C" fn(i64) -> usize = std::mem::transmute(base + 0xa6e4);
    let slot = index(WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as i64);
    // The original 0x6b08 meter update reads the first float of this HUD record.
    (slot < 8 && *((base + 0x57808 + slot * 0x1c) as *const f32) >= 50.0).then_some((base, slot))
}
pub(crate) unsafe fn has_charge(b: *mut app::BattleObjectModuleAccessor) -> bool {
    charged_slot(b).is_some()
}
pub(crate) unsafe fn spend_charge(b: *mut app::BattleObjectModuleAccessor) -> bool {
    let Some((base, slot)) = charged_slot(b) else { return false };
    let update: unsafe extern "C" fn(usize, f32) = std::mem::transmute(base + 0x6b08);
    update(slot, -50.0);
    true
}
unsafe fn reset_meter(b: *mut app::BattleObjectModuleAccessor) {
    let base = core();
    if base == 0 {
        return;
    }
    meter_add(b, 0.0);
    let index_fn: unsafe extern "C" fn(i64) -> usize = std::mem::transmute(base + 0xa6e4);
    let i = index_fn(WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as i64);
    if i < 8 {
        let subtract: unsafe extern "C" fn(usize, f32) = std::mem::transmute(base + 0x6b08);
        subtract(i, -1000.0);
        if app::smashball::is_training_mode() {
            let add: unsafe extern "C" fn(usize, f32) = std::mem::transmute(base + 0x6b08);
            add(i, 100.0);
        }
    }
}
unsafe fn opponent_lr(b: *mut app::BattleObjectModuleAccessor) -> f32 {
    if !is_one_on_one() {
        return 0.0;
    }
    let manager = singletons::FighterManager();
    let own = (*b).battle_object_id;
    let x = PostureModule::pos_x(b);
    for i in 0..8 {
        let entry = FighterManager::get_fighter_entry(manager, FighterEntryID(i));
        if entry == 0 {
            continue;
        }
        let id = FighterEntry::current_fighter_id(entry as *mut app::FighterEntry) as u32;
        if id == own
            || id == *BATTLE_OBJECT_ID_INVALID as u32
            || !app::sv_battle_object::is_active(id)
        {
            continue;
        }
        let other = app::sv_battle_object::module_accessor(id);
        if other.is_null() {
            continue;
        }
        let dx = PostureModule::pos_x(other) - x;
        if dx.abs() > 0.001 {
            return dx.signum();
        }
    }
    0.0
}
unsafe extern "C" fn frame(f: &mut L2CFighterCommon) {
    let b = f.module_accessor;
    if !slayer(b) {
        return;
    }
    let base = core();
    if base == 0 {
        return;
    }
    let status = StatusModule::status_kind(b);
    let Some(st) = state(b) else {
        return;
    };
    let previous = st.previous_status;
    st.previous_status = status;
    if !st.started || (status == *FIGHTER_STATUS_KIND_REBIRTH && previous != status) {
        st.ints = [0; 7];
        st.floats = [0.0; 0x72];
        st.flags = [false; 0x1b];
        st.started = true;
        st.last_damage = DamageModule::damage(b, 0);
        reset_meter(b);
    }
    // Mode entraînement : comme Potemkin, la provocation remplit la jauge (à la fin de la provocation,
    // sauf si elle a été annulée en Super Mappa).
    if previous == *FIGHTER_STATUS_KIND_APPEAL
        && status != *FIGHTER_STATUS_KIND_APPEAL
        && status != *FIGHTER_STATUS_KIND_SPECIAL_N
        && app::smashball::is_training_mode()
    {
        meter_add(b, 150.0);
    }
    // The original target-selection field is kept in private per-object storage, never Captain's smaller native work array.
    let compact: unsafe extern "C" fn(i64) -> usize = std::mem::transmute(base + 0xa6e4);
    let index = compact(WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as i64);
    if index < 8 {
        *((base + 0x579e8 + index) as *mut u8) =
            if FighterManager::is_result_mode(singletons::FighterManager()) {
                0
            } else {
                1
            };
    }
    *((base + 0x578f0) as *mut f32) = app::sv_information::slow_rate();
    *((base + 0x578f4) as *mut f32) = 1.0;
    let current_damage = DamageModule::damage(b, 0);
    let damage_gain = {
        let s = state(b).unwrap();
        let change = (current_damage - s.last_damage).max(0.0);
        s.last_damage = current_damage;
        change * 0.2
    };
    if damage_gain > 0.0 {
        meter_add(b, damage_gain);
    }
    let facing = opponent_lr(b);
    WorkModule::set_float(b, facing, 0x4c);
    if !app::sv_information::is_ready_go()
        || status == *FIGHTER_STATUS_KIND_DEAD
        || status == *FIGHTER_STATUS_KIND_WIN
    {
        return;
    }
    // Original ground eligible-state checks and original turn status; airborne rules from its main-line callback.
    if StatusModule::situation_kind(b) == *SITUATION_KIND_GROUND {
        let _ = original_status(0xa200, f);
    } else if [0xb, 0xc, 0xe, 0xf].contains(&status)
        && facing != 0.0
        && facing != PostureModule::lr(b)
        && MotionModule::motion_kind(b) != smash::hash40("air_turn")
    {
        StatusModule::change_status_request(b, 0x208, false);
    }
    let input: unsafe extern "C" fn(&mut L2CFighterCommon) = std::mem::transmute(base + 0x8f9c);
    input(f);
}
// Record the exact hitbox index at the game's collision event, as used by the source callback's AttackData lookup.
#[skyline::hook(offset = 0x46ae84, inline)]
unsafe fn hitbox_event(ctx: &InlineCtx) {
    let data = ctx.registers[1].x() as *const u32;
    if data.is_null() {
        return;
    }
    let id = *data;
    if !app::sv_battle_object::is_active(id) {
        return;
    }
    let b = app::sv_battle_object::module_accessor(id);
    if slayer(b) {
        if let Some(s) = state(b) {
            s.last_hitbox = *data.add(1) as i32;
        }
    }
}
// Correctif compteur de combo : ce crochet n'est plus posé au démarrage du jeu. Posé au démarrage, il modifiait
// le début de la fonction « X a touché Y » avant que combo_hud ne la cherche par sa signature : combo_hud ne
// trouvait plus la bonne fonction et ne comptait plus aucun coup. Il est maintenant posé une seule fois, au
// chargement du premier match avec Slayer (toutes les recherches des autres plugins sont faites depuis longtemps).
static COLLISION_HOOKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
unsafe extern "C" fn install_collision_once(_f: &mut L2CFighterCommon) {
    if !COLLISION_HOOKED.swap(true, Ordering::AcqRel) {
        skyline::install_hook!(collision);
    }
}
#[skyline::hook(offset = 0x67a7b0)]
unsafe fn collision(
    manager: u64,
    attacker: u32,
    defender: u32,
    move_type: u64,
    arg5: u64,
    again: u64,
) -> u64 {
    let result = original!()(manager, attacker, defender, move_type, arg5, again);
    if !app::sv_battle_object::is_active(attacker) || !app::sv_battle_object::is_active(defender) {
        return result;
    }
    let a = app::sv_battle_object::module_accessor(attacker);
    let d = app::sv_battle_object::module_accessor(defender);
    if a.is_null()
        || d.is_null()
        || app::utility::get_category(&mut *d) != *BATTLE_OBJECT_CATEGORY_FIGHTER
    {
        return result;
    }
    if slayer(a) {
        super::normals::record_contact(a, d);
        super::dandy::collision(a,d);
        let status = StatusModule::status_kind(a);
        if let Some(hitbox) = state(a).map(|s| s.last_hitbox) {
            if status == RC {
                // This is the original on-hit slowdown call, with the source work fields and exact engine arguments.
                let active = WorkModule::is_flag(a, 0x20000114);
                let delayed = WorkModule::is_flag(a, 0x20000113);
                if active || delayed {
                    let strength = if active {
                        WorkModule::get_float(a, 0x70) as i32
                    } else {
                        255
                    };
                    let frames =
                        WorkModule::get_int(a, if active { 0x100000cb } else { 0x100000c7 });
                    if active {
                        let pos = Vector3f {
                            x: 0.0,
                            y: 23.0,
                            z: 0.0,
                        };
                        let rot = Vector3f {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        };
                        for effect in ["rc_clock_base", "rc_clock"] {
                            EffectModule::req_follow(
                                d,
                                crate::slot_effect(a, effect),
                                Hash40::new("top"),
                                &pos,
                                &rot,
                                0.3,
                                true,
                                0x800000,
                                0,
                                -1,
                                0,
                                0,
                                false,
                                false,
                            );
                            if frames > 0 {
                                EffectModule::set_rate_last(d, 60.0 / frames as f32);
                            }
                        }
                    }
                    let fresh_reaction = active && WorkModule::is_flag(a, 0x200000fc) && !has_hit_reaction(d);
                    SlowModule::set(d, 0, strength, frames, false, 0x50000000);
                    if fresh_reaction {
                        WorkModule::set_float(d, 0.7, 0x1000003);
                        StatusModule::change_status_request_from_script(
                            d,
                            if StatusModule::situation_kind(d) == *SITUATION_KIND_GROUND {
                                *FIGHTER_STATUS_KIND_DAMAGE
                            } else {
                                *FIGHTER_STATUS_KIND_DAMAGE_AIR
                            },
                            false,
                        );
                    }
                }
            } else if MotionModule::motion_kind(a) != smash::hash40("slayer_super_mappa") && ![
                0x1fc, 0x1f9, 0x1f8, 0x1f7, 0x1f3, 0x1fa, 0x1fb, 0x1ef, 0x1f1, 0x1f2, 0x1e0,
            ]
            .contains(&status)
            {
                let data = AttackModule::attack_data(a, hitbox, false);
                if !data.is_null() {
                    meter_add(a, (*data).power * 0.75);
                }
            }
        }
    }
    if slayer(d) {
        let damage = DamageModule::damage(d, 0);
        if let Some(s) = state(d) {
            let gain = (damage - s.last_damage).max(0.0) * 0.2;
            s.last_damage = damage;
            if gain > 0.0 {
                meter_add(d, gain);
            }
        }
    }
    result
}

unsafe fn has_hit_reaction(b: *mut app::BattleObjectModuleAccessor) -> bool {
    if StopModule::is_stop(b) {
        return true;
    }
    [StatusModule::status_kind(b), StatusModule::status_kind_next(b)].into_iter().any(|status| {
        (*FIGHTER_STATUS_KIND_DAMAGE..=*FIGHTER_STATUS_KIND_DAMAGE_FALL).contains(&status)
            || status == *FIGHTER_STATUS_KIND_DAMAGE_FLY_REFLECT_JUMP_BOARD
            || status == *FIGHTER_STATUS_KIND_GUARD_DAMAGE
            || status == *FIGHTER_STATUS_KIND_DOWN
            || status == *FIGHTER_STATUS_KIND_DOWN_WAIT
            || status == *FIGHTER_STATUS_KIND_DOWN_DAMAGE
    })
}
pub fn install() {
    unsafe {
        skyline::install_hooks!(
            get_float,
            set_float,
            get_int,
            get_int64,
            set_int,
            dec_int,
            is_flag,
            on_flag,
            off_flag,
            set_flag,
            hitbox_event,
            original_tension_hud
        );
    }
    Agent::new("captain")
        .set_costume(crate::slots())
        .on_start(install_collision_once)
        .status(Pre, RC, common_pre)
        .status(Main, RC, rc_main)
        .status(End, RC, rc_end)
        .status(Pre, 0x201, common_pre)
        .status(Main, 0x201, ground_main)
        .status(End, 0x201, ground_end)
        .status(Pre, 0x208, common_pre)
        .status(Main, 0x208, air_main)
        .status(End, 0x208, air_end)
        .game_acmd("game_romancancel", roman_acmd, Priority::High)
        .game_acmd("game_romancancelair", roman_acmd, Priority::High)
        .on_line(Main, frame)
        .install();
}
