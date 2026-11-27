#![feature(proc_macro_hygiene)]
#![allow(unused, non_snake_case, non_upper_case_globals, static_mut_refs, unused_unsafe, unused_must_use)]

//! Slayer (GGST, mod de vigomasias) sur Captain Falcon, costume c180 : ses 4 lancers en deux temps.
//! Son animation de lancer (e01throwb, la même pour les 4) : uppercut vers le haut à l'image 3, puis coup de pied
//! vers le haut aux images 32-35. 1er temps : l'uppercut frappe l'adversaire tenu sans le lâcher (hitbox
//! « seulement sur l'adversaire tenu », sur l'os throw). 2e temps : le coup de pied l'éjecte vers le haut.
//! La position de l'adversaire vient de l'os Throw, animé dans e01throwb (fichier d'animation du mod).

use smash::app::sv_animcmd::{frame, wait};
use smash::app::lua_bind::*;
use smash::app::*;
use smash::lib::lua_const::*;
use smash::lua2cpp::*;
use smash::phx::Hash40;
use smash_script::macros::*;
use smash_script::*;
use smashline::*;

const COSTUMES: [usize; 1] = [180];
/// image du coup de pied qui éjecte
const F_KICK: f32 = 33.0;

unsafe extern "C" fn game_throw(agent: &mut L2CAgentBase) {
    let lua = agent.lua_state_agent;
    let boma = agent.module_accessor;
    if is_excute(agent) {
        // éjection du coup de pied : vers le haut
        smash_script::macros::ATTACK_ABS(agent, *FIGHTER_ATTACK_ABSOLUTE_KIND_THROW, 0, 7.0, 88, 75, 0, 70, 0.0, 1.0, *ATTACK_LR_CHECK_F, 0.0, true,
                   Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_KICK, *ATTACK_REGION_THROW);
        // si la prise est coupée (un autre joueur frappe Slayer), l'adversaire est relâché normalement
        smash_script::macros::ATTACK_ABS(agent, *FIGHTER_ATTACK_ABSOLUTE_KIND_CATCH, 0, 3.0, 361, 100, 0, 60, 0.0, 1.0, *ATTACK_LR_CHECK_F, 0.0, true,
                   Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_S, *COLLISION_SOUND_ATTR_NONE, *ATTACK_REGION_THROW);
    }
    frame(lua, 3.0);
    if is_excute(agent) {
        // 1er temps : l'uppercut touche l'adversaire tenu (4 %), sans recul : il reste dans la prise
        ATTACK(agent, 0, 0, Hash40::new("throw"), 4.0, 361, 0, 0, 0, 7.0, 0.0, 0.0, 0.0, None, None, None, 1.0, 1.0,
               *ATTACK_SETOFF_KIND_OFF, *ATTACK_LR_CHECK_F, false, 0, 0.0, 0, false, false, false, false, true,
               *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_FIGHTER, *COLLISION_PART_MASK_ALL, false,
               Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_M, *COLLISION_SOUND_ATTR_PUNCH, *ATTACK_REGION_PUNCH);
        AttackModule::set_catch_only_all(boma, true, false);
    }
    wait(lua, 2.0);
    if is_excute(agent) {
        AttackModule::clear_all(boma);
    }
    frame(lua, F_KICK);
    if is_excute(agent) {
        // 2e temps : le coup de pied éjecte
        let target = WorkModule::get_int64(boma, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_OBJECT);
        let group = WorkModule::get_int64(boma, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_GROUP);
        let no = WorkModule::get_int64(boma, *FIGHTER_STATUS_THROW_WORK_INT_TARGET_HIT_NO);
        ATK_HIT_ABS(agent, *FIGHTER_ATTACK_ABSOLUTE_KIND_THROW, Hash40::new("throw"), target, group, no);
    }
}

#[skyline::main(name = "slayer_prises")]
pub fn main() {
    let mut a = Agent::new("captain");
    a.set_costume(COSTUMES.to_vec());
    for s in ["game_throwf", "game_throwb", "game_throwhi", "game_throwlw"] {
        a.acmd(s, game_throw, Priority::Low);
    }
    a.install();
}
