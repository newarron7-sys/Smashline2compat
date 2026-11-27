//! Modif (demandée) : les 4 lancers en deux temps. Son animation de lancer (e01throwb, la même pour les 4) :
//! uppercut vers le haut à l'image 3, puis coup de pied vers le haut aux images 32-35.
//! 1er temps : l'uppercut frappe l'adversaire tenu sans le lâcher (hitbox seulement sur lui, sur l'os throw).
//! 2e temps : le coup de pied l'éjecte vers le haut. L'adversaire suit l'os Throw, animé dans e01throwb.
use smash::{app::lua_bind::*, lib::lua_const::*, lua2cpp::L2CAgentBase, phx::Hash40};
use smash::app::sv_animcmd::{frame, wait};
use smash_script::macros;
use smashline::{Agent, Priority};

const F_KICK: f32 = 33.0;

unsafe extern "C" fn throw(a: &mut L2CAgentBase) {
    let b = a.module_accessor;
    if macros::is_excute(a) {
        macros::ATTACK_ABS(a, *FIGHTER_ATTACK_ABSOLUTE_KIND_THROW, 0, 7.0, 88, 75, 0, 70, 0.0, 1.0, *ATTACK_LR_CHECK_F, 0.0, true,
            Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_KICK, *ATTACK_REGION_THROW);
        macros::ATTACK_ABS(a, *FIGHTER_ATTACK_ABSOLUTE_KIND_CATCH, 0, 3.0, 361, 100, 0, 60, 0.0, 1.0, *ATTACK_LR_CHECK_F, 0.0, true,
            Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_S, *COLLISION_SOUND_ATTR_NONE, *ATTACK_REGION_THROW);
    }
    frame(a.lua_state_agent, 3.0);
    if macros::is_excute(a) {
        macros::ATTACK(a, 0, 0, Hash40::new("throw"), 4.0, 361, 0, 0, 0, 7.0, 0.0, 0.0, 0.0, None, None, None, 1.0, 1.0,
            *ATTACK_SETOFF_KIND_OFF, *ATTACK_LR_CHECK_F, false, 0, 0.0, 0, false, false, false, false, true,
            *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_FIGHTER, *COLLISION_PART_MASK_ALL, false,
            Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_M, *COLLISION_SOUND_ATTR_PUNCH, *ATTACK_REGION_PUNCH);
        AttackModule::set_catch_only_all(b, true, false);
    }
    wait(a.lua_state_agent, 2.0);
    if macros::is_excute(a) {
        AttackModule::clear_all(b);
    }
    frame(a.lua_state_agent, F_KICK);
    if macros::is_excute(a) {
        let target = WorkModule::get_int64(b, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT);
        let group = WorkModule::get_int64(b, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP);
        let no = WorkModule::get_int64(b, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_NO);
        macros::ATK_HIT_ABS(a, *FIGHTER_ATTACK_ABSOLUTE_KIND_THROW, Hash40::new("throw"), target, group, no);
    }
}

pub fn install() {
    Agent::new("captain").set_costume(crate::slots())
        .game_acmd("game_throwf", throw, Priority::High)
        .game_acmd("game_throwb", throw, Priority::High)
        .game_acmd("game_throwhi", throw, Priority::High)
        .game_acmd("game_throwlw", throw, Priority::High)
        .install();
}

// Journal (temporaire) : où se trouvent l'os throw de Slayer et l'adversaire tenu, pendant la prise, les lancers
// et le down B. Écrit dans sd:/slayer_prise_log.txt (au plus 400 lignes par lancement du jeu).
static mut NB_LIGNES: u32 = 0;
static mut IMAGE: u32 = 0;
fn journal(m: &str) {
    unsafe {
        if NB_LIGNES >= 400 { return; }
        NB_LIGNES += 1;
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("sd:/slayer_prise_log.txt") {
        let _ = writeln!(f, "{}", m);
    }
}
unsafe fn joint(b: *mut smash::app::BattleObjectModuleAccessor, nom: &str) -> (f32, f32, f32) {
    let mut p = smash::phx::Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    ModelModule::joint_global_position(b, Hash40::new(nom), &mut p, true);
    (p.x, p.y, p.z)
}
unsafe extern "C" fn suivi(f: &mut smash::lua2cpp::L2CFighterCommon) {
    let b = f.module_accessor;
    if !crate::is_slot(WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR)) { return; }
    IMAGE = IMAGE.wrapping_add(1);
    if IMAGE % 8 != 0 || !LinkModule::is_linked(b, *LINK_NO_CAPTURE) { return; }
    let id = LinkModule::get_node_object_id(b, *LINK_NO_CAPTURE) as u32;
    if !smash::app::sv_battle_object::is_active(id) { return; }
    let v = smash::app::sv_battle_object::module_accessor(id);
    if v.is_null() { return; }
    let p = PostureModule::pos(b);
    let q = PostureModule::pos(v);
    let t = joint(b, "throw");
    let h = joint(v, "hip");
    let cible = if LinkModule::is_model_constraint(v) { LinkModule::get_model_constraint_target_joint(v) } else { 0 };
    journal(&format!(
        "statut {:#x} image {:.0} | slayer ({:.1},{:.1}) os throw ({:.1},{:.1}) | adversaire statut {:#x} pos ({:.1},{:.1}) hanche ({:.1},{:.1}) | joint cible {:#x} (throw={:#x})",
        StatusModule::status_kind(b), MotionModule::frame(b), (*p).x, (*p).y, t.0, t.1,
        StatusModule::status_kind(v), (*q).x, (*q).y, h.0, h.1, cible, Hash40::new("throw").hash));
}
pub fn install_suivi() {
    Agent::new("captain").set_costume(crate::slots())
        .on_line(smashline::Main, suivi)
        .install();
}
