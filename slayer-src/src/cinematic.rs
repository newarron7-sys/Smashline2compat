use smash::{
    app::{
        self, ArticleOperationTarget, GroundCliffCheckKind, HitStatus, SituationKind, lua_bind::*,
    },
    lib::{L2CValue, lua_const::*},
    lua2cpp::{L2CFighterCommon, L2CWeaponCommon},
    phx::Hash40,
};
use smashline::{Agent, Main, Pre};
use std::sync::atomic::{AtomicI32, Ordering};
static HEAD_ARTICLE: AtomicI32 = AtomicI32::new(-1);
unsafe fn cinematic_status(boma: *mut app::BattleObjectModuleAccessor) -> bool {
    let status = StatusModule::status_kind(boma);
    status == *FIGHTER_STATUS_KIND_WIN
        || status == *FIGHTER_STATUS_KIND_FINAL
        || (status >= *FIGHTER_CAPTAIN_STATUS_KIND_FINAL_RIDE
            && status <= *FIGHTER_CAPTAIN_STATUS_KIND_FINAL_END)
}
pub(crate) unsafe fn low_head_visible(b: *mut app::BattleObjectModuleAccessor) -> bool {
    let article = HEAD_ARTICLE.load(Ordering::Relaxed);
    !super::locomotion::teleport_active(b)
        && (!cinematic_status(b) || article < 0 || !ArticleModule::is_exist(b, article))
}
unsafe extern "C" fn fighter_frame(f: &mut L2CFighterCommon) {
    let b = f.module_accessor;
    if !crate::is_slot(WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR)) {
        return;
    }
    let article = HEAD_ARTICLE.load(Ordering::Relaxed);
    if article < 0 {
        return;
    }
    let cine = cinematic_status(b);
    if cine && !ArticleModule::is_exist(b, article) {
        ArticleModule::generate_article(b, article, false, -1);
    }
    let exists = ArticleModule::is_exist(b, article);
    if !cine && exists {
        ArticleModule::remove_exist(b, article, ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL));
    }
    let show_low = low_head_visible(b);
    for mesh in [
        "HeadLow_Base",
        "HeadLow_Eyes",
        "HeadLow_Glass",
        "headlow_base",
        "headlow_eyes",
        "headlow_glass",
    ] {
        ModelModule::set_mesh_visibility(b, Hash40::new(mesh), show_low);
    }
    super::locomotion::outline_pair(
        b,
        "HeadLow_Outline",
        super::locomotion::mirror_inverted(b),
        show_low,
    );
}
unsafe extern "C" fn head_pre(w: &mut L2CWeaponCommon) -> L2CValue {
    StatusModule::init_settings(
        w.module_accessor,
        SituationKind(*SITUATION_KIND_NONE),
        *WEAPON_KINETIC_TYPE_NONE,
        *GROUND_CORRECT_KIND_NONE as u32,
        GroundCliffCheckKind(0),
        false,
        0,
        0,
        0,
        0,
    );
    0.into()
}
unsafe extern "C" fn head_main(w: &mut L2CWeaponCommon) -> L2CValue {
    let b = w.module_accessor;
    AttackModule::clear_all(b);
    HitModule::set_whole(b, HitStatus(*HIT_STATUS_OFF), 0);
    MotionModule::change_motion(b, Hash40::new("wait"), 0.0, 0.0, false, 0.0, false, false);
    LinkModule::set_model_constraint_pos_ort(
        b,
        *WEAPON_LINK_NO_CONSTRAINT,
        Hash40::new("head_high_root"),
        Hash40::new("g_head_attach"),
        *CONSTRAINT_FLAG_MTX as u32,
        true,
    );
    super::locomotion::outline_pair(b, "Head_Outline", false, true);
    head_loop(w);
    w.fastshift(L2CValue::Ptr(head_loop as *const () as _))
}
unsafe extern "C" fn head_loop(w: &mut L2CWeaponCommon) -> L2CValue {
    let b = w.module_accessor;
    AttackModule::clear_all(b);
    let id = WorkModule::get_int(b, *WEAPON_INSTANCE_WORK_ID_INT_LINK_OWNER) as u32;
    if app::sv_battle_object::is_active(id) {
        let owner = app::sv_battle_object::module_accessor(id);
        if !owner.is_null() {
            super::locomotion::outline_pair(
                b,
                "Head_Outline",
                super::locomotion::mirror_inverted(owner),
                true,
            );
        }
    }
    0.into()
}
pub fn install() {
    unsafe {
        let relative = smashline::clone_weapon(
            "peach",
            *WEAPON_KIND_PEACH_KINOPIO,
            "captain",
            "slayer_head_high",
            false,
        );
        HEAD_ARTICLE.store(2 + relative, Ordering::Relaxed);
        Agent::new("captain_slayer_head_high")
            .set_costume(crate::slots())
            .status(Pre, 0, head_pre)
            .status(Main, 0, head_main)
            .install();
        Agent::new("captain")
            .set_costume(crate::slots())
            .on_line(Main, fighter_frame)
            .install();
    }
}
