//! Keep Captain's hurtbox groups and status handling on Slayer's animated rig.
use smash::{
    app::{self, lua_bind::*},
    lib::lua_const::*,
    lua2cpp::L2CFighterCommon,
    phx::{Hash40, Vector3f},
};
use smashline::Agent;

static mut APPLIED: [(u32, bool); 8] = [(u32::MAX, false); 8];

unsafe extern "C" fn start(f: &mut L2CFighterCommon) {
    if let Some(index) = super::locomotion::entry(f.module_accessor) {
        APPLIED[index] = (u32::MAX, false);
    }
}

unsafe extern "C" fn update(f: &mut L2CFighterCommon) {
    let b = f.module_accessor;
    let Some(entry) = super::locomotion::entry(b) else { return };
    let rebirth = StatusModule::status_kind(b) == *FIGHTER_STATUS_KIND_REBIRTH;
    let state = ((*b).battle_object_id, rebirth);
    if APPLIED[entry] == state {
        return;
    }
    if APPLIED[entry].0 == state.0 && !rebirth {
        APPLIED[entry] = state;
        return;
    }

    let body = (*COLLISION_PART_BODY, *HIT_HEIGHT_CENTER);
    let head = (*COLLISION_PART_HEAD, *HIT_HEIGHT_HIGH);
    let leg = (*COLLISION_PART_BODY_LEGS, *HIT_HEIGHT_LOW);
    let capsules = [
        ("G_Head_Attach", 2.65, [0.0, 0.0, 0.25], [0.0, 2.4, 0.25], head),
        ("G_pelvis", 3.0, [0.0, -1.4, 0.0], [0.0, 0.3, 0.0], body),
        ("G_chest", 3.05, [0.0, -1.2, 0.0], [0.0, 1.1, 0.0], body),
        ("G_uparm_L", 1.55, [0.0, 0.0, 0.0], [2.7, 0.0, 0.0], body),
        ("G_uparm_R", 1.55, [0.0, 0.0, 0.0], [-2.7, 0.0, 0.0], body),
        ("G_lowarm_L", 1.35, [0.0, 0.0, 0.0], [4.15, 0.0, 0.0], body),
        ("G_lowarm_R", 1.35, [0.0, 0.0, 0.0], [-4.15, 0.0, 0.0], body),
        ("G_thigh_L", 1.75, [0.0, 0.0, 0.0], [0.0, -4.3, 0.0], leg),
        ("G_thigh_R", 1.75, [0.0, 0.0, 0.0], [0.0, -4.3, 0.0], leg),
        ("G_calf_L", 1.4, [0.0, 0.0, 0.0], [0.0, -5.5, 0.0], leg),
        ("G_calf_R", 1.4, [0.0, 0.0, 0.0], [0.0, -5.5, 0.0], leg),
        ("G_foot_L", 1.15, [0.0, 0.0, 0.0], [0.0, -0.65, 1.4], leg),
        ("G_foot_R", 1.15, [0.0, 0.0, 0.0], [0.0, -0.65, 1.4], leg),
    ];
    let count = HitModule::get_part_size(b, 0) as usize;
    if count == 0 || count > 64 {
        return;
    }
    for (index, &(bone, radius, [ax, ay, az], [bx, by, bz], (part, height))) in
        capsules.iter().take(count).enumerate()
    {
        let index = index as i32;
        let status = HitModule::get_status(b, index, 0);
        app::FighterUtil::set_hit_data(
            b,
            index,
            0,
            &Vector3f { x: ax, y: ay, z: az },
            &Vector3f { x: bx, y: by, z: bz },
            radius,
            Hash40::new(bone),
            app::CollisionPart(part),
            app::HitHeight(height),
            app::HitStatus(status),
            app::CollisionShapeType(*COLLISION_SHAPE_TYPE_CAPSULE),
        );
    }
    APPLIED[entry] = state;
    skyline::println!("[Slayer defense] {} limb hurtboxes", count.min(capsules.len()));
}

pub fn install() {
    Agent::new("captain")
        .set_costume(crate::slots())
        .on_start(start)
        .on_line(smashline::Main, update)
        .install();
}
