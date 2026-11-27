//! Source mesh effects: cosmetic only, synchronized to motion frames on hit and whiff.
use smash::{app::{self,ArticleOperationTarget,GroundCliffCheckKind,HitStatus,SituationKind,lua_bind::*},lib::{L2CValue,lua_const::*},lua2cpp::{L2CFighterCommon,L2CWeaponCommon},phx::{Hash40,Vector3f}};
use smashline::{Agent,Main,Pre};
use std::sync::atomic::{AtomicI32,Ordering};
static FX_ARTICLE:AtomicI32=AtomicI32::new(-1);
// Per fighter entry: (motion kind, frames since it began). Looping owner motions (down_wait) play their source burst
// once on this clock instead of replaying it every loop.
static mut CLOCK:[(u64,f32);8]=[(0,0.0);8];
// v0.3.47: per fighter entry, (effect clip, frame) when the owner left a tail clip at its natural end. GGST's teleport
// smoke outlives the owner motion, so the actor plays the rest of that clip on its own clock.
static mut TAIL:[(u64,f32);8]=[(0,-1.0);8];
fn tail_clip(kind:u64)->bool {["fx_14_r","fx_14_l","fx_15_r","fx_15_l","fx_31_r","fx_31_l","fx_32_r","fx_32_l","fx_36_r","fx_36_l"].iter().any(|name|kind==Hash40::new(name).hash)}
unsafe fn play_tail(b:*mut app::BattleObjectModuleAccessor,e:usize)->L2CValue {
 let (kind,frame)=TAIL[e];let next=frame+1.0;
 if tail_clip(kind)&&MotionModule::motion_kind(b)==kind&&next<=MotionModule::end_frame(b) {
     TAIL[e].1=next;VisibilityModule::set_whole(b,true);MotionModule::set_frame(b,next,true);
 } else {TAIL[e]=(0,-1.0);VisibilityModule::set_whole(b,false);}
 0.into()
}
// A started 2H slash keeps its source lifetime after the owner cancels.
static mut SWEEP_TAIL: [(u32, u64, f32); 8] = [(0, 0, -1.0); 8];

unsafe fn sweep_tail(
    b: *mut app::BattleObjectModuleAccessor,
    owner: *mut app::BattleObjectModuleAccessor,
    entry: usize,
) -> bool {
    let id = (*owner).battle_object_id;
    let motion = MotionModule::motion_kind(owner);
    let status = StatusModule::status_kind(owner);
    let (previous_id, kind, frame) = SWEEP_TAIL[entry];
    if previous_id != id || [*FIGHTER_STATUS_KIND_DEAD, *FIGHTER_STATUS_KIND_REBIRTH].contains(&status) {
        SWEEP_TAIL[entry] = (id, 0, -1.0);
        return false;
    }
    if motion == smash::hash40("attack_lw4") {
        let frame = MotionModule::frame(owner);
        let kind = if super::locomotion::mirror_inverted(owner) { "fx_42_l" } else { "fx_42_r" };
        SWEEP_TAIL[entry] = (id, smash::hash40(kind), if (12.0..35.0).contains(&frame) { frame } else { -1.0 });
        return false;
    }
    if frame < 0.0 {
        return false;
    }
    let next = if StopModule::is_stop(owner) { frame } else { frame + 1.0 };
    if next >= 35.0 {
        SWEEP_TAIL[entry] = (id, 0, -1.0);
        return false;
    }
    SWEEP_TAIL[entry].2 = next;
    VisibilityModule::set_whole(b, true);
    if MotionModule::motion_kind(b) != kind {
        MotionModule::change_motion(b, Hash40::new_raw(kind), next, 0.0, false, 0.0, false, false);
    } else if MotionModule::frame(b) != next {
        MotionModule::set_frame(b, next, true);
    }
    true
}
#[derive(Clone, Copy)]
struct LaunchOrigin {
    motion: u64,
    frame: f32,
    position: Vector3f,
    captured: bool,
}
static mut LAUNCH: [LaunchOrigin; 8] = [LaunchOrigin {
    motion: 0, frame: 0.0, position: Vector3f { x: 0.0, y: 0.0, z: 0.0 }, captured: false,
}; 8];

fn wild_assault(motion: u64) -> bool {
    ["attack_s4_s", "attack_s4_hi", "attack_s4_lw"].iter().any(|name| motion == Hash40::new(name).hash)
}

unsafe fn track_launch(b: *mut app::BattleObjectModuleAccessor, entry: usize, motion: u64) {
    let frame = MotionModule::frame(b);
    let origin = &mut LAUNCH[entry];
    if origin.motion != motion || frame < origin.frame || frame < 7.0 {
        origin.captured = false;
    }
    origin.motion = motion;
    origin.frame = frame;
    if wild_assault(motion) && frame >= 7.0 && !origin.captured {
        ModelModule::joint_global_position(b, Hash40::new("trans"), &mut origin.position, true);
        origin.captured = true;
    }
}

unsafe fn place_launch_ring(b: *mut app::BattleObjectModuleAccessor, owner: *mut app::BattleObjectModuleAccessor) {
    let Some(entry) = super::locomotion::entry(owner) else { return; };
    let origin = LAUNCH[entry];
    if !origin.captured { return; }
    let mut current = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    ModelModule::joint_global_position(owner, Hash40::new("trans"), &mut current, true);
    let scale = PostureModule::scale(owner).max(0.01);
    let position = Vector3f {
        x: 0.0,
        y: 8.9 + (origin.position.y - current.y) / scale,
        z: -2.0 + (origin.position.x - current.x) * PostureModule::lr(owner) / scale,
    };
    // The Wild Assault clip reserves fx_003 for its ring. The slash uses another joint.
    ModelModule::set_joint_translate(b, Hash40::new("fx_003"), &position, false, false);
}

unsafe fn once(kind:u64)->bool {kind==Hash40::new("down_wait_u").hash||kind==Hash40::new("down_wait_d").hash}
unsafe fn owner_frame(owner:*mut app::BattleObjectModuleAccessor)->f32 {
 let kind=MotionModule::motion_kind(owner);
 if once(kind){if let Some(e)=super::locomotion::entry(owner){if CLOCK[e].0==kind{return CLOCK[e].1;}}}
 MotionModule::frame(owner)
}
const BLOOD_PARTICLES: [(&str, &str); 12] = [
    ("SLY_BU_PARTICLE_00", "sly_bu_particle_00"), ("SLY_BU_PARTICLE_01", "sly_bu_particle_01"),
    ("SLY_BU_PARTICLE_02", "sly_bu_particle_02"), ("SLY_BU_PARTICLE_03", "sly_bu_particle_03"),
    ("SLY_BU_PARTICLE_04", "sly_bu_particle_04"), ("SLY_BU_PARTICLE_05", "sly_bu_particle_05"),
    ("SLY_BU_PARTICLE_06", "sly_bu_particle_06"), ("SLY_BU_PARTICLE_07", "sly_bu_particle_07"),
    ("SLY_BU_PARTICLE_08", "sly_bu_particle_08"), ("SLY_BU_PARTICLE_09", "sly_bu_particle_09"),
    ("SLY_BU_PARTICLE_10", "sly_bu_particle_10"), ("SLY_BU_PARTICLE_11", "sly_bu_particle_11"),
];

const FLOOR_ANCHORS: [&str; 2] = ["sly_bg_launch", "sly_bg_stop"];
const MAPPA_P_DASH: &[i32] = &[8];
const MAPPA_K_DASH: &[i32] = &[5, 13, 22];
const SWEEP_DASH: &[i32] = &[6];
const MAPPA_P_BRAKE: &[i32] = &[21];
const MAPPA_K_BRAKE: &[i32] = &[26];
const SWEEP_BRAKE: &[i32] = &[18];
// v0.3.78: source bg_dash / bg_dashbrake (CreateParticle frame - 1, the pose clock). Dandy P's second brake (source 38)
// falls after the 34-frame Smash motion; Dandy K has no brake in source.
const DANDY_P_DASH: &[i32] = &[4];
const DANDY_P_BRAKE: &[i32] = &[13];
const DANDY_K_DASH: &[i32] = &[0, 5, 17, 21];
const DANDY_K_BRAKE: &[i32] = &[];
const SUPER_DASH: &[i32] = &[156, 161];
const SUPER_BRAKE: &[i32] = &[167, 174];

#[derive(Clone, Copy)]
struct FloorFx {
    motion: u64,
    frame: f32,
    cues: [i32; 2],
    origins: [Option<Vector3f>; 2],
}

impl FloorFx {
    const EMPTY: Self = Self { motion: 0, frame: 0.0, cues: [-1; 2], origins: [None; 2] };
}

static mut FLOOR: [FloorFx; 8] = [FloorFx::EMPTY; 8];

fn floor_cues(motion: u64) -> Option<[&'static [i32]; 2]> {
    if motion == smash::hash40("slayer_mappa_p") {
        Some([MAPPA_P_DASH, MAPPA_P_BRAKE])
    } else if motion == smash::hash40("slayer_mappa_k") {
        Some([MAPPA_K_DASH, MAPPA_K_BRAKE])
    } else if motion == smash::hash40("slayer_2d") {
        Some([SWEEP_DASH, SWEEP_BRAKE])
    } else if motion == smash::hash40("slayer_dandy_p") {
        Some([DANDY_P_DASH, DANDY_P_BRAKE])
    } else if motion == smash::hash40("slayer_dandy_k") {
        Some([DANDY_K_DASH, DANDY_K_BRAKE])
    } else if motion == smash::hash40("slayer_super_mappa") {
        Some([SUPER_DASH, SUPER_BRAKE])
    } else {
        None
    }
}

unsafe fn record_floor(owner: *mut app::BattleObjectModuleAccessor, entry: usize, motion: u64, frame: f32) {
    let Some(cues) = floor_cues(motion) else {
        return;
    };
    let floor = &mut FLOOR[entry];
    if floor.motion != motion || frame < floor.frame {
        *floor = FloorFx { motion, ..FloorFx::EMPTY };
    }
    floor.frame = frame;
    for (i, times) in cues.into_iter().enumerate() {
        let Some(cue) = times.iter().rev().find(|&&cue| frame >= cue as f32) else { continue; };
        if floor.cues[i] == *cue {
            continue;
        }
        floor.cues[i] = *cue;
        floor.origins[i] = if StatusModule::situation_kind(owner) == *SITUATION_KIND_GROUND {
            let mut position = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
            ModelModule::joint_global_position(owner, Hash40::new("trans"), &mut position, true);
            Some(position)
        } else {
            None
        };
    }
}

unsafe fn place_floor(b: *mut app::BattleObjectModuleAccessor, owner: *mut app::BattleObjectModuleAccessor, entry: usize) {
    let mut current = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
    ModelModule::joint_global_position(owner, Hash40::new("trans"), &mut current, true);
    let scale = PostureModule::scale(owner).max(0.01);
    for (name, origin) in FLOOR_ANCHORS.into_iter().zip(FLOOR[entry].origins) {
        let joint = Hash40::new(name);
        let visible = if origin.is_some() { 1.0 } else { 0.0 };
        ModelModule::set_joint_scale(b, joint, &Vector3f { x: visible, y: visible, z: visible });
        if let Some(origin) = origin {
            ModelModule::set_joint_translate(b, joint, &Vector3f {
                x: 0.0, y: (origin.y - current.y) / scale,
                z: (origin.x - current.x) * PostureModule::lr(owner) / scale,
            }, false, false);
        }
    }
}

unsafe fn aura_slot(
    b: *mut app::BattleObjectModuleAccessor, joint: &str, size: f32, x: f32, y: f32, z: f32,
) {
    let joint = Hash40::new(joint);
    ModelModule::set_joint_scale(b, joint, &Vector3f { x: size, y: size, z: size });
    if size > 0.0 {
        ModelModule::set_joint_translate(b, joint, &Vector3f { x, y, z }, false, false);
    }
}

unsafe fn universe_aura(b: *mut app::BattleObjectModuleAccessor, owner: *mut app::BattleObjectModuleAccessor) {
    let (phase, ending) = super::downb::universe_phase(owner);
    for (index, (mesh, joint)) in BLOOD_PARTICLES.into_iter().enumerate() {
        let (mut size, mut x, mut y, mut z) = (0.0, 0.0, 0.0, 0.0);
        if let Some(age) = ending {
            let seed = ((index * 37 + 13) % 97) as f32 / 96.0;
            let height = ((index * 61 + 11) % 97) as f32 / 96.0;
            let life = 30.0 + seed * 18.0;
            let flow = (age / life).min(1.0);
            size = (0.30 + 0.40 * seed) * (1.0 + 0.60 * flow)
                * (flow * 10.0).min(1.0) * ((1.0 - flow) * 5.0).min(1.0);
            x = seed * 2.0;
            y = 2.0 + 14.0 * height + (4.0 + seed * 3.0) * age / 60.0;
            z = (2.0 * seed - 1.0) * 4.0;
        } else if let Some(clock) = phase {
            let upper = index < 6;
            let slot = index % 6;
            let emission = clock + (slot * 7 + if upper { 0 } else { 3 }) as f32;
            let age = emission % 42.0;
            let cycle = (emission / 42.0) as u32;
            let seed = ((cycle * 17 + index as u32 * 23) % 97) as f32 / 96.0;
            let lifetime = 31.0 + seed * 10.0;
            if age < lifetime {
                let flow = age / lifetime;
                let rise = 4.0 + seed * if upper { 6.0 } else { 3.0 };
                let height = if upper { 13.5 + 2.5 * seed } else { 8.0 + 2.0 * seed };
                let spread = if upper { 4.0 } else { 3.5 };
                z = (seed * 2.0 - 1.0) * spread;
                y = height + rise * age / 60.0;
                size = ((if upper { 0.4 } else { 0.3 }) + seed * 0.4)
                    * (flow * 12.0).min(1.0) * ((1.0 - flow) * 5.0).min(1.0);
            }
        }
        aura_slot(b, joint, size, x, y, z);
        ModelModule::set_mesh_visibility(b, Hash40::new(mesh), size > 0.0);
    }
    let mut haze_visible = false;
    for (index, joint) in ["sly_bu_haze_upper", "sly_bu_haze_lower"].into_iter().enumerate() {
        // Strive's ambient haze is a delayed burst, independent of the
        // repeating shell fragments. The ending has an immediate burst.
        let age = ending.or_else(|| phase.filter(|clock| *clock >= 30.0).map(|clock| clock - 30.0));
        let progress = age.map_or(1.0, |age| (age / 38.0).clamp(0.0, 1.0));
        let envelope = (progress * 9.0).min(1.0) * ((1.0 - progress) * 3.0).min(1.0);
        let base = if index == 0 { 1.0 } else { 0.65 };
        let height = if index == 0 { 16.0 } else { 6.5 };
        let size = base * (1.0 + 0.3 * progress) * envelope;
        haze_visible |= size > 0.0;
        aura_slot(b, joint, size, 2.5, height + 0.9 * progress, if index == 0 { 1.7 } else { -1.5 });
    }
    ModelModule::set_mesh_visibility(b, Hash40::new("SLY_BU_HAZE"), haze_visible);
    if phase.is_some() || ending.is_some() {
        VisibilityModule::set_whole(b, true);
    }
}
unsafe fn idle_fx(b: *mut app::BattleObjectModuleAccessor, owner: *mut app::BattleObjectModuleAccessor, e: usize) -> L2CValue {
    play_tail(b, e);
    if TAIL[e].1 >= 0.0 && floor_cues(FLOOR[e].motion).is_some()
        && ["fx_14_r", "fx_14_l", "fx_15_r", "fx_15_l", "fx_36_r", "fx_36_l"].iter().any(|name| TAIL[e].0 == Hash40::new(name).hash)
    {
        place_floor(b, owner, e);
    }
    let (phase, ending) = super::downb::universe_phase(owner);
    if (phase.is_some() || ending.is_some()) && TAIL[e].1 < 0.0
        && MotionModule::motion_kind(b) != Hash40::new("wait").hash {
        MotionModule::change_motion(b, Hash40::new("wait"), 0.0, 0.0, false, 0.0, false, false);
    }
    ModelModule::set_mesh_visibility(b, Hash40::new("SLY_2K_SHOE_SMEAR"), false);
    universe_aura(b, owner);
    0.into()
}
unsafe fn profile(owner:*mut app::BattleObjectModuleAccessor)->Option<(&'static str,&'static str,f32,f32)> {
 let kind=MotionModule::motion_kind(owner);
 if kind==Hash40::new("attack_11").hash {return Some(("fx_00_r","fx_00_l",4.0,12.0));}
 if ["attack_12", "attack_s3_s", "attack_s3_hi"].iter().any(|name| kind == Hash40::new(name).hash) {
     return Some(("fx_44_r", "fx_44_l", 6.0, 42.0));
 }
 if kind==Hash40::new("attack_hi3").hash {return Some(("fx_45_r","fx_45_l",6.0,31.0));}
 if kind==Hash40::new("attack_dash").hash {return Some(("fx_02_r","fx_02_l",18.0,26.0));}
 if wild_assault(kind) {return Some(("fx_41_r","fx_41_l",7.0,30.0));}
 if kind==Hash40::new("slayer_fs").hash {return Some(("fx_03_r","fx_03_l",7.0,52.0));}
  if kind==Hash40::new("attack_lw3").hash {return Some(("fx_05_r","fx_05_l",0.0,31.0));}
 if kind==Hash40::new("slayer_6h_tilt").hash {return Some(("fx_06_r","fx_06_l",16.0,26.0));}
 if kind==Hash40::new("attack_hi4").hash {return Some(("fx_07_r","fx_07_l",3.0,24.0));}
 if kind==Hash40::new("attack_lw4").hash {return Some(("fx_08_r","fx_08_l",0.0,35.0));}
 if kind==Hash40::new("attack_air_n").hash {return Some(("fx_09_r","fx_09_l",4.0,7.0));}
 if kind==Hash40::new("attack_air_f").hash {return Some(("fx_10_r","fx_10_l",5.0,12.0));}
 if kind==Hash40::new("attack_air_b").hash {return Some(("fx_11_r","fx_11_l",10.0,33.0));}
 if kind==Hash40::new("attack_air_hi").hash {return Some(("fx_12_r","fx_12_l",8.0,14.0));}
 if kind==Hash40::new("attack_air_lw").hash {return Some(("fx_13_r","fx_13_l",8.0,21.0));}
 if kind==Hash40::new("slayer_super_mappa").hash {return Some(("fx_43_r","fx_43_l",16.0,209.0));}
 if kind==Hash40::new("slayer_mappa_p").hash {return Some(("fx_14_r","fx_14_l",2.0,31.0));}
 if kind==Hash40::new("slayer_mappa_k").hash {return Some(("fx_15_r","fx_15_l",2.0,39.0));}
 if kind==Hash40::new("slayer_dandy_p").hash {return Some(("fx_16_r","fx_16_l",4.0,34.0));}
 if kind==Hash40::new("slayer_dandy_k").hash {return Some(("fx_17_r","fx_17_l",0.0,40.0));}
 if kind==Hash40::new("slayer_pilebunker").hash {return Some(("fx_19_r","fx_19_l",4.0,28.0));}
 if kind==Hash40::new("slayer_bumpahead").hash {return Some(("fx_20_r","fx_20_l",12.0,56.0));}
 if kind==Hash40::new("slayer_itslate").hash {return Some(("fx_21_r","fx_21_l",17.0,34.0));}
 if kind==Hash40::new("slayer_hammer_air").hash {return Some(("fx_22_r","fx_22_l",19.0,82.0));}
 if kind==Hash40::new("slayer_hammer_land").hash {return Some(("fx_23_r","fx_23_l",0.0,18.0));}
 if kind==Hash40::new("attack_s3_lw").hash {return Some(("fx_04_r","fx_04_l",0.0,41.0));}
 if kind==Hash40::new("slayer_bu").hash {return Some(("fx_28_r","fx_28_l",8.0,20.0));}
 if kind==Hash40::new("slayer_bu_exe").hash {return Some(("fx_29_r","fx_29_l",44.0,86.0));}
 if kind==Hash40::new("slayer_airbite_exe").hash {return Some(("fx_30_r","fx_30_l",13.0,47.0));}
 if kind==Hash40::new("slayer_teleport_f").hash {return Some(("fx_31_r","fx_31_l",0.0,18.0));}
 if kind==Hash40::new("slayer_teleport_b").hash {return Some(("fx_32_r","fx_32_l",0.0,28.0));}
 if kind==Hash40::new("squat").hash {return Some(("fx_33_r","fx_33_l",0.0,8.0));}
 if kind==Hash40::new("squat_wait").hash || kind==Hash40::new("squat_wait_item").hash {return Some(("fx_34_r","fx_34_l",0.0,97.0));}
 if kind==Hash40::new("squat_rv").hash {return Some(("fx_35_r","fx_35_l",0.0,10.0));}
 if kind==Hash40::new("slayer_2d").hash {return Some(("fx_36_r","fx_36_l",0.0,44.0));}
 if kind==Hash40::new("down_bound_u").hash||kind==Hash40::new("down_bound_d").hash {return Some(("fx_37_r","fx_37_l",9.0,15.0));}
 // v0.3.42: the knockdown bats live their seeded source lifetimes (up to frame 64 of down_wait).
 if once(kind) {return Some(("fx_38_r","fx_38_l",0.0,65.0));}
 if ["slayer_airdash_f","escape_air_slide"].iter().any(|name|kind==Hash40::new(name).hash) {return Some(("fx_39_r","fx_39_l",0.0,24.0));}
 if kind==Hash40::new("slayer_airdash_b").hash {return Some(("fx_40_r","fx_40_l",0.0,11.0));}
 None
}
unsafe extern "C" fn fighter_frame(f:&mut L2CFighterCommon) {
 let b=f.module_accessor;
 let Some(e)=super::locomotion::entry(b) else {return;};
 let kind=MotionModule::motion_kind(b);
 track_launch(b,e,kind);
 if CLOCK[e].0 != kind {
     CLOCK[e] = (kind, 0.0);
 } else if !StopModule::is_stop(b) {
     CLOCK[e].1 += 1.0;
 }
 let article=FX_ARTICLE.load(Ordering::Relaxed);if article<0{return;}
 let visible=profile(b).map(|(_,_,_start,end)|{let frame=owner_frame(b);frame>=0.0&&frame<end}).unwrap_or(false)
     || super::downb::universe_armed(b);
 let exists = ArticleModule::is_exist(b, article);
 let status = StatusModule::status_kind(b);
 if [*FIGHTER_STATUS_KIND_DEAD, *FIGHTER_STATUS_KIND_REBIRTH].contains(&status) {
     FLOOR[e] = FloorFx::EMPTY;
     if exists { ArticleModule::remove_exist(b, article, ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL)); }
 } else if visible && !exists {
     // Keep one actor for the stock. Its status loop hides it between effects.
     ArticleModule::generate_article(b, article, false, -1);
 }
}
unsafe extern "C" fn fx_pre(w:&mut L2CWeaponCommon)->L2CValue {
 StatusModule::init_settings(w.module_accessor,SituationKind(*SITUATION_KIND_NONE),*WEAPON_KINETIC_TYPE_NONE,*GROUND_CORRECT_KIND_NONE as u32,GroundCliffCheckKind(0),false,0,0,0,0);0.into()
}
unsafe extern "C" fn fx_main(w:&mut L2CWeaponCommon)->L2CValue {
 let b=w.module_accessor;AttackModule::clear_all(b);HitModule::set_whole(b,HitStatus(*HIT_STATUS_OFF),0);
 MotionModule::change_motion(b,Hash40::new("wait"),0.0,0.0,false,0.0,false,false);
 LinkModule::set_model_constraint_pos_ort(b,*WEAPON_LINK_NO_CONSTRAINT,Hash40::new("move_fx_root"),Hash40::new("trans"),*CONSTRAINT_FLAG_MTX as u32,true);
 fx_loop(w);w.fastshift(L2CValue::Ptr(fx_loop as *const () as _))
}
unsafe extern "C" fn fx_loop(w:&mut L2CWeaponCommon)->L2CValue {
 let b=w.module_accessor;
 let id=WorkModule::get_int(b,*WEAPON_INSTANCE_WORK_ID_INT_LINK_OWNER) as u32;
 if !app::sv_battle_object::is_active(id){VisibilityModule::set_whole(b,false);return 0.into();}
 let owner=app::sv_battle_object::module_accessor(id);
 let Some(e)=super::locomotion::entry(owner) else {VisibilityModule::set_whole(b,false);return 0.into();};
 if sweep_tail(b, owner, e) {
  ModelModule::set_mesh_visibility(b, Hash40::new("SLY_2K_SHOE_SMEAR"), false);
  universe_aura(b, owner);
  return 0.into();
 }
 if let Some((right,left,start,end))=profile(owner){
  let frame=owner_frame(owner);let wanted=Hash40::new(if super::locomotion::mirror_inverted(owner){left}else{right});
  if frame < start || frame >= end {
      return idle_fx(b, owner, e);
  }
  VisibilityModule::set_whole(b,true);
  if MotionModule::motion_kind(b)!=wanted.hash {
      if ["fx_41_r", "fx_41_l"].iter().any(|name| MotionModule::motion_kind(b) == Hash40::new(name).hash) {
          ModelModule::clear_joint_srt(b, Hash40::new("fx_003"));
      }
      MotionModule::change_motion(b,wanted,frame,0.0,false,0.0,false,false);
  } else if MotionModule::frame(b)!=frame {
      MotionModule::set_frame(b,frame,true);
  }
  let motion = MotionModule::motion_kind(owner);
  ModelModule::set_mesh_visibility(b, Hash40::new("SLY_2K_SHOE_SMEAR"),
      motion == smash::hash40("attack_lw3") && (5.0..7.0).contains(&frame));
  if wild_assault(motion) { place_launch_ring(b,owner); }
  if floor_cues(motion).is_some() {
      record_floor(owner, e, motion, frame);
      place_floor(b, owner, e);
  } else {
      ModelModule::clear_joint_srt(b, Hash40::new(FLOOR_ANCHORS[0]));
  }
  // Near the natural end only (the owner status may switch before this loop sees the last frame).
  TAIL[e]=if frame>=end-3.0 {(wanted.hash,frame)} else {(0,-1.0)};
 } else {
  return idle_fx(b, owner, e);
 }
 universe_aura(b, owner);
 0.into()
}
pub fn install(){unsafe{
 let relative=smashline::clone_weapon("mario",*WEAPON_KIND_MARIO_MANTLE,"captain","slayer_move_fx",false);FX_ARTICLE.store(2+relative,Ordering::Relaxed);
 Agent::new("captain_slayer_move_fx").set_costume(crate::slots()).status(Pre,0,fx_pre).status(Main,0,fx_main).install();
 Agent::new("captain").set_costume(crate::slots()).on_line(Main,fighter_frame).install();
}}
