#![feature(proc_macro_hygiene)]
use std::collections::HashMap;
use smash::hash40;
extern crate the_csk_collection_api as csk;
mod moves;
mod shake;
mod native_fx;
mod hitstop;
mod audio;
mod normals;
mod normals_math;
mod cinematic;
mod mechanics;
mod effect_lookup;
mod locomotion;
mod movement_math;
mod mappa;
mod downb;
mod defense;
mod mappa_math;
mod dandy;
mod dandy_math;
mod slayer_effects;
mod physical_fx;
mod throws;
/// Slayer's colour slots: c180 is GGST Color 1, c181-c195 Colors 2-16, c196 the Haunted colour (source Color90).
pub(crate) const FIRST_SLOT: i32 = 180;
pub(crate) const LAST_SLOT: i32 = 180; // seul c180 est installé (1 skin) : c181-c196 restent libres pour d autres persos sur Captain Falcon
pub(crate) fn slots() -> Vec<usize> {
    (FIRST_SLOT as usize..=LAST_SLOT as usize).collect()
}
pub(crate) fn param_slots() -> Vec<i32> {
    (FIRST_SLOT..=LAST_SLOT).collect()
}
pub(crate) fn is_slot(color: i32) -> bool {
    (FIRST_SLOT..=LAST_SLOT).contains(&color)
}
/// one-slot-effects registers the shared bank once per slot as <name>_c<slot>.
pub(crate) unsafe fn slot_effect(b: *mut smash::app::BattleObjectModuleAccessor, name: &str) -> smash::phx::Hash40 {
    let color = smash::app::lua_bind::WorkModule::get_int(b, *smash::lib::lua_const::FIGHTER_INSTANCE_WORK_ID_INT_COLOR);
    effect_slot_hash(color, name)
}
pub(crate) fn effect_slot_hash(color: i32, name: &str) -> smash::phx::Hash40 {
    let color = if is_slot(color) { color } else { FIRST_SLOT } as u8;
    let mut bytes = [0u8; 64];
    let len = name.len();
    assert!(len + 5 <= bytes.len());
    bytes[..len].copy_from_slice(name.as_bytes());
    bytes[len..len + 5].copy_from_slice(&[b'_', b'c', b'0' + color / 100, b'0' + color / 10 % 10, b'0' + color % 10]);
    smash::phx::Hash40::new(std::str::from_utf8(&bytes[..len + 5]).expect("ASCII effect slot suffix"))
}
fn register_css() {
    let series = hash40("ui_series_guiltygear");
    csk::add_series_db_entry_info(csk::SeriesDatabaseEntry {
        ui_series_id: series,
        name_id: csk::StringType::Overwrite(csk::CStrCSK::new("guiltygear")),
        ..Default::default()
    });
    csk::add_chara_db_entry_info(csk::CharacterDatabaseEntry {
        ui_chara_id: hash40("ui_chara_slayer"),
        clone_from_ui_chara_id: Some(hash40("ui_chara_captain")),
        name_id: csk::StringType::Overwrite(csk::CStrCSK::new("slayer")),
        ui_series_id: csk::Hash40Type::Overwrite(series),
        fighter_kind: csk::Hash40Type::Overwrite(hash40("fighter_kind_captain")),
        fighter_kind_corps: csk::Hash40Type::Overwrite(hash40("fighter_kind_captain")),
        alt_chara_id: csk::Hash40Type::Overwrite(0x2302D482A /* Hash40 of -1 */),
        shop_item_tag: csk::Hash40Type::Overwrite(0x2302D482A /* Hash40 of -1 */),
        disp_order: csk::SignedByteType::Optional(Some(-1)),
        chara_count: csk::SignedByteType::Overwrite(1),
        can_select: csk::BoolType::Overwrite(true),
        is_usable_soundtest: csk::BoolType::Overwrite(false),
        is_called_pokemon: csk::BoolType::Overwrite(false),
        is_dlc: csk::BoolType::Overwrite(false),
        is_patch: csk::BoolType::Overwrite(false),
        color_num: csk::UnsignedByteType::Overwrite((LAST_SLOT - FIRST_SLOT + 1) as u8),
        extra_index_maps: csk::UnsignedByteMap::Overwrite(HashMap::from([
            (hash40("color_start_index"), csk::UnsignedByteType::Overwrite(FIRST_SLOT as u8)),
        ])),
        extra_hash_maps: csk::Hash40Map::Overwrite(HashMap::from([
            (hash40("original_ui_chara_hash"), csk::Hash40Type::Overwrite(hash40("ui_chara_slayer"))),
            (hash40("characall_label"), csk::Hash40Type::Overwrite(hash40("vc_narration_characall_slayer"))),
        ])),
        ..Default::default()
    });

    // One layout per colour; every colour uses the same art framing, so all clone captain's first layout.
    for color in 0..=(LAST_SLOT - FIRST_SLOT) {
        csk::add_chara_layout_db_entry_info(csk::CharacterLayoutDatabaseEntry {
            ui_layout_id: hash40(&format!("ui_chara_slayer_{:02}", color)),
            clone_from_ui_layout_id: Some(hash40("ui_chara_captain_00")),
            ui_chara_id: csk::Hash40Type::Overwrite(hash40("ui_chara_slayer")),
            ..Default::default()
        });
    }
}

// Print exported addresses once so a guest crash's caller can be attributed
// across ASLR without logging or allocating from per-frame hooks.
fn log_crash_symbols() {
    for (name, symbol) in [
        ("one-slot make_effect", &b"make_effect\0"[..]),
        ("CSK csk_collection_version", &b"csk_collection_version\0"[..]),
    ] {
        let mut address = 0usize;
        unsafe { skyline::nn::ro::LookupSymbol(&mut address, symbol.as_ptr()); }
        if address != 0 {
            skyline::println!("[Slayer crash trace] {} = {:#x}", name, address);
        }
    }
}

#[skyline::main(name = "slayer_smash")]
pub fn main() { log_crash_symbols(); unsafe { effect_lookup::install(); param_config::update_int(*smash::lib::lua_const::FIGHTER_KIND_CAPTAIN, param_slots(), (hash40("attack_combo_max"),0), 1); } register_css(); moves::install(); audio::install(); normals::install(); cinematic::install(); mechanics::install(); locomotion::install(); mappa::install(); downb::install(); defense::install(); dandy::install(); slayer_effects::install(); physical_fx::install(); throws::install(); }
