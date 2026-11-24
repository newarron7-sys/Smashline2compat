//! Compatibilité Smashline 2.0 -> Smash 13.0.4 + Smashline 1.6.6 (celui du user).
//!
//! Smashline 2.0 ne tourne que sur 13.0.5. Les plugins faits pour lui (Potemkin v3, King Boo)
//! sont modifiés pour chercher `smashlinx_*` au lieu de ces fonctions, qui n'existent pas
//! (ou pas avec ces paramètres) dans le Smashline 1.6.6 :
//!   - smashline_clone_weapon (format 2.0)       -> traduit vers le clone_weapon 1.6.6
//!   - smashline_update_weapon_count (format 2.0) -> nombre max des clones, appliqué par
//!     slx_generate_article (le plugin du mod appelle celle-ci à la place de generate_article)
//!   - smashline_enable_weapon_module            -> écrit dans la table des modules du jeu
//!   - smashline_replace_weapon_vtable           -> hook get_weapon_vtable + aiguillage par joueur
//!   - smashline_replace_fighter_vtable_costume  -> hook get_fighter_vtable, aiguillage par costume
//!   - smashline_get_original_fighter_vtable     -> entrée d'origine de la vtable
//!
//! Reprend libkingboo_compat (testé en jeu sur 13.0.4) en le généralisant à plusieurs persos.
//! Ne pas installer les deux en même temps : celui-ci remplace libkingboo_compat.nro.
//!
//! Adresses 13.0.4 : get_fighter_vtable 0x68d530 et get_weapon_vtable 0x33be790 sont aussi celles
//! du Potemkin de septembre 2026 (fait pour 13.0.4) ; les numéros d'entrée de vtable de Potemkin v3
//! (0, 4, 7, 12, 13, 30, 36, 43, 45, 46, 48, 50, 59, 68) sont identiques à ceux de cette version.
#![allow(static_mut_refs)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const WEAPON_MODULE_TABLE: usize = 0x5189818; // table des fabriques de modules (EXO 13.0.4)
const WEAPON_MODULE_ROW_SIZE: usize = 0xe8;
const ENTRY_INFO_VEC: usize = 0x5323680; // Smashline 13.0.4 get_costume_from_entry_id

const FIGHTER_VTABLE_SLOTS: usize = 147;
const WEAPON_VTABLE_SLOTS: usize = 104;
const NO_COSTUME: i32 = -1;

/// Fabriques des modules d'arme, offsets 13.0.4 (Smashline 2.0 - 0x5B0).
const WEAPON_MODULE_FACTORIES: [Option<usize>; 29] = [
    Some(0x33b89a0 - 0x5b0), // 0  KineticModule
    Some(0x33b97b0 - 0x5b0), // 1  ArticleModule
    Some(0x33b8af0 - 0x5b0), // 2  AttackModule
    Some(0x33b95b0 - 0x5b0), // 3  ControlModule
    Some(0x33b8ce0 - 0x5b0), // 4  EffectModule
    Some(0x33b8e00 - 0x5b0), // 5  GroundModule
    Some(0x33b8e90 - 0x5b0), // 6  HitModule
    Some(0x33b8300 - 0x5b0), // 7  MotionModule
    Some(0x33b8f80 - 0x5b0), // 8  ReflectModule
    Some(0x33b9030 - 0x5b0), // 9  SearchModule
    Some(0x33b9210 - 0x5b0), // 10 SoundModule
    Some(0x33b9a20 - 0x5b0), // 11 VisibilityModule
    Some(0x33b8510 - 0x5b0), // 12 ColorBlendModule
    Some(0x33b85a0 - 0x5b0), // 13 ShakeModule
    Some(0x33ba5c0 - 0x5b0), // 14 ItemModule
    Some(0x33b9500 - 0x5b0), // 15 AreaModule
    Some(0x33b9cf0 - 0x5b0), // 16 JostleModule
    Some(0x33b9c40 - 0x5b0), // 17 CameraModule
    Some(0x33b9ae0 - 0x5b0), // 18 PhysicsModule
    Some(0x33b9360 - 0x5b0), // 19 DamageModule
    Some(0x33b9fa0 - 0x5b0), // 20 CatchModule
    Some(0x33ba060 - 0x5b0), // 21 GrabModule
    Some(0x33b9720 - 0x5b0), // 22 SlopeModule
    Some(0x33b9de0 - 0x5b0), // 23 ReflectorModule
    Some(0x33b9900 - 0x5b0), // 24 SlowModule
    Some(0x33b8780 - 0x5b0), // 25 MotionAnimcmdModule
    Some(0x33b9ef0 - 0x5b0), // 26 TurnModule
    Some(0x33b93f0 - 0x5b0), // 27 LuaModule
    None,                    // 28 InkPaintModule
];

fn text() -> usize {
    unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize }
}

// ---------------------------------------------------------------- types FFI (identiques à Smashline)

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StringFFI {
    ptr: *const u8,
    len: usize,
}

impl StringFFI {
    /// Comme Smashline : la chaîne est copiée sur le tas et « fuitée ». Celui qui la reçoit
    /// (le plugin Smashline) en devient propriétaire et la libère lui-même.
    fn from_str(s: &str) -> Self {
        let mut owned = s.to_string();
        owned.shrink_to_fit();
        let leaked = owned.into_bytes().leak();
        Self { ptr: leaked.as_ptr(), len: leaked.len() }
    }
    unsafe fn as_string(&self) -> String {
        if self.ptr.is_null() {
            return String::new();
        }
        String::from_utf8_lossy(std::slice::from_raw_parts(self.ptr, self.len)).into_owned()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Costume {
    data: *const usize,
    len: usize,
}

impl Costume {
    unsafe fn to_vec(&self) -> Vec<i32> {
        if self.data.is_null() || self.len == 0 {
            return vec![];
        }
        std::slice::from_raw_parts(self.data, self.len).iter().map(|c| *c as i32).collect()
    }
}

#[repr(C)]
pub struct CloneWeapon {
    pub generate_id: i32,
    pub weapon_id: i32,
}

fn lookup(name: &str) -> usize {
    let mut addr = 0usize;
    let c = format!("{}\0", name);
    let res = unsafe { skyline::nn::ro::LookupSymbol(&mut addr, c.as_ptr() as _) };
    if res != 0 || addr == 0 {
        panic!("[smashline2_compat] '{}' introuvable : le plugin Smashline doit être installé", name);
    }
    addr
}

// ---------------------------------------------------------------- tables des persos gérés

/// Propriétaire d'origine de chaque arme clonée (WEAPON_KIND_* -> nom du combattant).
fn original_owner_of(weapon_kind: i32) -> Option<&'static str> {
    Some(match weapon_kind {
        // King Boo
        0x6 => "miiswordsman",   // WEAPON_KIND_MIISWORDSMAN_WAVE
        0x6b => "daisy",         // WEAPON_KIND_DAISY_KINOPIO
        0x1a4 => "krool",        // WEAPON_KIND_KROOL_IRONBALL
        0x1e9 => "packun",       // WEAPON_KIND_PACKUN_POISONBREATH
        0x20a => "dolly",        // WEAPON_KIND_DOLLY_CAP
        // Potemkin
        0x16c => "cloud",        // WEAPON_KIND_CLOUD_WAVE        -> kamui_fdb
        0x175 => "kamui",        // WEAPON_KIND_KAMUI_DRAGONHAND  -> kamui_effect
        0x176 => "kamui",        // WEAPON_KIND_KAMUI_SPEARHAND   -> kamui_heavenly
        0x177 => "kamui",        // WEAPON_KIND_KAMUI_WATERDRAGON -> kamui_weapon
        _ => return None,
    })
}

/// FIGHTER_KIND_* et nombre d'articles vanilla (= plus grand FIGHTER_*_GENERATE_ARTICLE_* + 1,
/// table MAX_GENERATE_ARTICLE_IDS de Smashline 2.0) des propriétaires de clones.
fn owner_info(owner: &str) -> Option<(i32, i32)> {
    Some(match owner {
        "dedede" => (0x2a, 8 + 1),
        "kamui" => (0x3f, 4 + 1),
        _ => return None,
    })
}

/// WEAPON_KIND (d'origine) cloné -> FIGHTER_KIND du nouveau propriétaire.
static CLONED_KINDS: Mutex<BTreeMap<i32, i32>> = Mutex::new(BTreeMap::new());

fn clone_owner_kind(weapon_kind: i32) -> Option<i32> {
    CLONED_KINDS.lock().unwrap().get(&weapon_kind).copied()
}

/// FIGHTER_KIND -> costumes du mod (ceux passés à replace_fighter_vtable_costume).
static COSTUMES: Mutex<BTreeMap<i32, Vec<i32>>> = Mutex::new(BTreeMap::new());

fn is_mod_costume(owner_kind: i32, costume: i32) -> bool {
    if costume == NO_COSTUME {
        return false;
    }
    match COSTUMES.lock().unwrap().get(&owner_kind) {
        Some(list) => list.is_empty() || list.contains(&costume),
        None => false,
    }
}

// ---------------------------------------------------------------- clone_weapon (2.0 -> 1.6.6)

#[no_mangle]
pub extern "C" fn smashlinx_clone_weapon(
    owner_name: StringFFI,
    article_name: StringFFI,
    original_article_id: i32,
    use_original_code: bool,
) -> CloneWeapon {
    let owner = unsafe { owner_name.as_string() };
    let article = unsafe { article_name.as_string() };
    let original_owner = original_owner_of(original_article_id).unwrap_or_else(|| {
        panic!("[smashline2_compat] arme d'origine {:#x} inconnue ({}_{})", original_article_id, owner, article)
    });
    let (owner_kind, base) = owner_info(&owner)
        .unwrap_or_else(|| panic!("[smashline2_compat] propriétaire '{}' non géré", owner));

    CLONED_KINDS.lock().unwrap().insert(original_article_id, owner_kind);

    // Smashline 1.6.6 : (original_owner, original_article_id, new_owner, new_name, use_original_code) -> index
    let old: extern "C" fn(StringFFI, i32, StringFFI, StringFFI, bool) -> i32 =
        unsafe { std::mem::transmute(lookup("smashline_clone_weapon")) };
    let index = old(
        StringFFI::from_str(original_owner),
        original_article_id,
        StringFFI::from_str(&owner),
        StringFFI::from_str(&article),
        use_original_code,
    );
    // Les clones sont ajoutés après les articles vanilla du propriétaire (même numérotation que 2.0).
    // Avec Smashline 1.6.6, le clone garde le WEAPON_KIND de l'arme d'origine.
    CloneWeapon { generate_id: base + index, weapon_id: original_article_id }
}

// ---------------------------------------------------------------- update_weapon_count (2.0)
//
// 2.0 : (fighter_kind, generate_article_id, new_count). Le 1.6.6 ne change que le nombre maximum
// des articles vanilla : ses clones recopient celui de l'arme d'origine. Sans correction, un clone
// peut exister en plusieurs exemplaires (up B de Potemkin : 3 generate_article d'affilée -> un 2e
// Giganter figé, conteneur visible).
//
// On n'intercepte PAS get_static_fighter_data : Smashline 1.6.6 le détourne déjà, et poser un
// 2e détournement dessus faisait boucler le jeu au chargement (crash du 24/11).
// À la place, le plugin du mod est modifié pour appeler slx_generate_article au lieu de
// ArticleModule::generate_article (même longueur de nom). Cette fonction refuse de créer un clone
// déjà présent en nombre maximum, comme le fait le jeu avec Smashline 2.0, puis appelle la vraie.

/// (FIGHTER_KIND, generate_article_id) -> nombre maximum voulu
static WEAPON_COUNTS: Mutex<BTreeMap<(i32, i32), i32>> = Mutex::new(BTreeMap::new());

#[no_mangle]
pub extern "C" fn smashlinx_update_weapon_count(fighter_kind: i32, generate_article_id: i32, new_count: i32) {
    WEAPON_COUNTS.lock().unwrap().insert((fighter_kind, generate_article_id), new_count);
}


extern "C" {
    #[link_name = "_ZN3app8lua_bind36ArticleModule__generate_article_implEPNS_26BattleObjectModuleAccessorEibi"]
    fn game_generate_article(accessor: *mut u8, article: i32, arg3: bool, arg4: i32) -> u64;
    #[link_name = "_ZN3app8lua_bind34ArticleModule__get_active_num_implEPNS_26BattleObjectModuleAccessorEi"]
    fn game_get_active_num(accessor: *mut u8, article: i32) -> i32;
    #[link_name = "_ZN3app7utility8get_kindEPKNS_26BattleObjectModuleAccessorE"]
    fn game_get_kind(accessor: *const u8) -> i32;
}

#[export_name = "_ZN3slx8lua_bind36ArticleModule__generate_article_implEPNS_26BattleObjectModuleAccessorEibi"]
pub extern "C" fn slx_generate_article(accessor: *mut u8, article: i32, arg3: bool, arg4: i32) -> u64 {
    let wanted: Vec<(i32, i32)> = WEAPON_COUNTS
        .lock()
        .unwrap()
        .iter()
        .filter(|((_, id), _)| *id == article)
        .map(|((k, _), n)| (*k, *n))
        .collect();
    unsafe {
        if !wanted.is_empty() && !accessor.is_null() {
            let kind = game_get_kind(accessor);
            if let Some((_, max)) = wanted.iter().find(|(k, _)| *k == kind) {
                if game_get_active_num(accessor, article) >= *max {
                    return 0;
                }
            }
        }
        game_generate_article(accessor, article, arg3, arg4)
    }
}

// ---------------------------------------------------------------- enable_weapon_module

#[no_mangle]
pub extern "C" fn smashlinx_enable_weapon_module(weapon_kind: i32, module_slot: usize) {
    if clone_owner_kind(weapon_kind).is_none() {
        return;
    }
    let Some(Some(factory)) = WEAPON_MODULE_FACTORIES.get(module_slot).copied() else {
        return;
    };
    // La table est indexée par le « kind de base » de l'arme, pas par son WEAPON_KIND
    // (ex. le Toad de Daisy utilise la ligne du Toad de Peach). On remplit les deux lignes.
    let base_kind = unsafe { get_weapon_base_kind(weapon_kind) };
    let mut rows = vec![weapon_kind];
    if base_kind >= 0 && base_kind != weapon_kind {
        rows.push(base_kind);
    }
    for row in rows {
        let entry = WEAPON_MODULE_TABLE + WEAPON_MODULE_ROW_SIZE * row as usize + module_slot * 8;
        let _ = skyline::patching::Patch::in_text(entry).data((text() + factory) as u64);
    }
}

/// 13.0.4 : 0x33b01b0 (Smashline 2.0, 13.0.5) - 0x5B0
#[skyline::from_offset(0x33afc00)]
fn get_weapon_base_kind(kind: i32) -> i32;

// ---------------------------------------------------------------- reconnaître le perso du mod
//
// On n'intercepte PAS fighter_initialize_object_data (0x6079d0) : le Smashline 1.6.6 s'en sert pour
// rediriger les projectiles clonés. Les entrées de vtable remplacées passent par un aiguillage qui
// vérifie, à chaque appel, si l'objet appartient au mod (bon combattant + costume du mod).

unsafe fn costume_from_entry_id(entry_id: i32) -> i32 {
    if !(0..8).contains(&entry_id) {
        return NO_COSTUME;
    }
    let text = text() as *const u64;
    let some_vec = *text.add(ENTRY_INFO_VEC / 8);
    if some_vec == 0 {
        return NO_COSTUME;
    }
    let some_struct = *((some_vec + entry_id as u64 * 8 + 0xe8) as *const u64);
    if some_struct == 0 {
        return NO_COSTUME;
    }
    *((some_struct + 100) as *const i32)
}

// ---------------------------------------------------------------- aiguillages (thunks)
//
// Chaque entrée de vtable remplacée pointe vers un aiguillage. Il sauvegarde tous les registres
// d'arguments (x0-x8, q0-q7), demande à slx_resolve quelle fonction appeler (mod ou d'origine)
// selon l'objet reçu en x1, restaure tout, puis saute à cette fonction : les arguments sont transmis
// intacts, quelle que soit la signature.

const THUNK_COUNT: usize = 64;
const MODE_FIGHTER: usize = 0;
const MODE_WEAPON: usize = 1;

/// Par aiguillage : [fonction du mod, fonction d'origine, mode, FIGHTER_KIND du propriétaire]
static mut SLX_THUNK_TABLE: [[usize; 4]; THUNK_COUNT] = [[0; 4]; THUNK_COUNT];
static NEXT_THUNK: AtomicUsize = AtomicUsize::new(0);

extern "C" fn slx_resolve(index: usize, object: *const u8) -> usize {
    // Uniquement des lectures mémoire (aucun appel au jeu) : l'aiguillage peut être appelé pendant
    // la construction de l'objet, quand ses modules ne sont pas encore prêts.
    unsafe {
        let [func, orig, mode, owner_kind] = SLX_THUNK_TABLE[index];
        if object.is_null() {
            return orig;
        }
        let kind = *(object.add(0xc) as *const i32);
        let entry_id = *(object.add(0x10) as *const i32);
        if mode == MODE_FIGHTER && kind != owner_kind as i32 {
            return orig;
        }
        if is_mod_costume(owner_kind as i32, costume_from_entry_id(entry_id)) { func } else { orig }
    }
}

fn make_thunk(func: usize, orig: usize, mode: usize, owner_kind: i32) -> Option<usize> {
    let index = NEXT_THUNK.fetch_add(1, Ordering::Relaxed);
    if index >= THUNK_COUNT {
        return None;
    }
    unsafe {
        SLX_THUNK_TABLE[index] = [func, orig, mode, owner_kind as usize];
    }
    Some(thunk_address(index))
}

fn thunk_address(index: usize) -> usize {
    extern "C" {
        fn slx_thunk_base();
    }
    // Chaque aiguillage fait exactement 8 octets (mov + b), à la suite de slx_thunk_base.
    slx_thunk_base as usize + index * 8
}

std::arch::global_asm!(
    ".text",
    ".p2align 3",
    "slx_thunk_common:",
    "    sub sp, sp, #0xe0",
    "    stp x0, x1, [sp, #0x00]",
    "    stp x2, x3, [sp, #0x10]",
    "    stp x4, x5, [sp, #0x20]",
    "    stp x6, x7, [sp, #0x30]",
    "    stp x8, x30, [sp, #0x40]",
    "    stp q0, q1, [sp, #0x60]",
    "    stp q2, q3, [sp, #0x80]",
    "    stp q4, q5, [sp, #0xa0]",
    "    stp q6, q7, [sp, #0xc0]",
    "    mov x0, x16",
    "    bl {resolve}",
    "    mov x16, x0",
    "    ldp q6, q7, [sp, #0xc0]",
    "    ldp q4, q5, [sp, #0xa0]",
    "    ldp q2, q3, [sp, #0x80]",
    "    ldp q0, q1, [sp, #0x60]",
    "    ldp x8, x30, [sp, #0x40]",
    "    ldp x6, x7, [sp, #0x30]",
    "    ldp x4, x5, [sp, #0x20]",
    "    ldp x2, x3, [sp, #0x10]",
    "    ldp x0, x1, [sp, #0x00]",
    "    add sp, sp, #0xe0",
    "    br x16",
    ".p2align 3",
    ".global slx_thunk_base",
    ".hidden slx_thunk_base",
    "slx_thunk_base:",
    ".irp i, 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63",
    "    mov x16, #\\i",
    "    b slx_thunk_common",
    ".endr",
    resolve = sym slx_resolve,
);

// ---------------------------------------------------------------- replace_fighter_vtable_costume

/// (FIGHTER_KIND, entrée) -> fonction du mod
static FIGHTER_UPDATES: Mutex<BTreeMap<(i32, usize), usize>> = Mutex::new(BTreeMap::new());
static FIGHTER_VTABLE_CACHE: Mutex<BTreeMap<i32, usize>> = Mutex::new(BTreeMap::new());
/// FIGHTER_KIND -> vptr de la vtable d'origine du jeu
static FIGHTER_ORIGINAL_VPTR: Mutex<BTreeMap<i32, usize>> = Mutex::new(BTreeMap::new());

#[repr(C)]
struct FighterVtable {
    vptr: *const usize,
    padding: [usize; 2],
    slots: [usize; FIGHTER_VTABLE_SLOTS],
}

#[repr(C)]
struct WeaponVtable {
    vptr: *const usize,
    padding: [usize; 2],
    slots: [usize; WEAPON_VTABLE_SLOTS],
}

#[no_mangle]
pub extern "C" fn smashlinx_replace_fighter_vtable_costume(
    fighter_kind: i32,
    costume: Costume,
    vtable_slot: usize,
    vtable_function: usize,
) {
    if vtable_slot >= FIGHTER_VTABLE_SLOTS {
        return;
    }
    let costumes = unsafe { costume.to_vec() };
    {
        let mut all = COSTUMES.lock().unwrap();
        let list = all.entry(fighter_kind).or_default();
        for c in &costumes {
            if !list.contains(c) {
                list.push(*c);
            }
        }
    }
    FIGHTER_UPDATES.lock().unwrap().insert((fighter_kind, vtable_slot), vtable_function);
    FIGHTER_VTABLE_CACHE.lock().unwrap().clear();
}

/// Entrée d'origine (jeu) de la vtable du combattant.
#[no_mangle]
pub extern "C" fn smashlinx_get_original_fighter_vtable(fighter_kind: i32, vtable_slot: usize) -> usize {
    if vtable_slot >= FIGHTER_VTABLE_SLOTS {
        return 0;
    }
    let known = FIGHTER_ORIGINAL_VPTR.lock().unwrap().get(&fighter_kind).copied();
    let vptr = match known {
        Some(vptr) => vptr,
        None => {
            // Passe par le hook ci-dessous, qui note la vtable d'origine de ce combattant.
            unsafe { game_get_fighter_vtable(fighter_kind) };
            match FIGHTER_ORIGINAL_VPTR.lock().unwrap().get(&fighter_kind).copied() {
                Some(vptr) => vptr,
                None => return 0,
            }
        }
    };
    unsafe { *(vptr as *const usize).add(vtable_slot) }
}

#[skyline::from_offset(0x68d530)]
fn game_get_fighter_vtable(kind: i32) -> *const usize;

#[skyline::hook(offset = 0x68d530)]
unsafe fn get_fighter_vtable(kind: i32) -> *const usize {
    if let Some(cached) = FIGHTER_VTABLE_CACHE.lock().unwrap().get(&kind) {
        return *cached as *const usize;
    }
    let original = call_original!(kind);
    if original.is_null() {
        return original;
    }
    let vptr = *(original as *const *const usize);
    FIGHTER_ORIGINAL_VPTR.lock().unwrap().insert(kind, vptr as usize);

    let merged: Vec<(usize, usize)> = FIGHTER_UPDATES
        .lock()
        .unwrap()
        .iter()
        .filter(|((k, _), _)| *k == kind)
        .map(|((_, slot), func)| (*slot, *func))
        .collect();
    if merged.is_empty() {
        return original;
    }

    let mut slots = [0usize; FIGHTER_VTABLE_SLOTS];
    std::ptr::copy_nonoverlapping(vptr, slots.as_mut_ptr(), FIGHTER_VTABLE_SLOTS);
    for (slot, func) in merged {
        if let Some(thunk) = make_thunk(func, slots[slot], MODE_FIGHTER, kind) {
            slots[slot] = thunk;
        }
    }
    let patched = Box::leak(Box::new(FighterVtable { vptr: std::ptr::null(), padding: [0; 2], slots }));
    patched.vptr = patched.slots.as_ptr();
    let addr = patched as *const FighterVtable as usize;
    FIGHTER_VTABLE_CACHE.lock().unwrap().insert(kind, addr);
    addr as *const usize
}

// ---------------------------------------------------------------- replace_weapon_vtable
//
// Avec Smashline 1.6.6, un clone partage le WEAPON_KIND de l'arme d'origine : les entrées remplacées
// passent par l'aiguillage, qui ne donne la fonction du mod qu'aux projectiles dont le propriétaire
// joue un costume du mod.

static WEAPON_UPDATES: Mutex<BTreeMap<i32, BTreeMap<usize, usize>>> = Mutex::new(BTreeMap::new());
static WEAPON_VTABLE_CACHE: Mutex<BTreeMap<i32, usize>> = Mutex::new(BTreeMap::new());

#[no_mangle]
pub extern "C" fn smashlinx_replace_weapon_vtable(weapon_kind: i32, vtable_slot: usize, vtable_function: usize) {
    if clone_owner_kind(weapon_kind).is_none() {
        return;
    }
    if vtable_slot >= WEAPON_VTABLE_SLOTS {
        return;
    }
    WEAPON_UPDATES
        .lock()
        .unwrap()
        .entry(weapon_kind)
        .or_default()
        .insert(vtable_slot, vtable_function);
    WEAPON_VTABLE_CACHE.lock().unwrap().clear();
}

#[skyline::hook(offset = 0x33be790)]
unsafe fn get_weapon_vtable(kind: i32) -> *const usize {
    if let Some(cached) = WEAPON_VTABLE_CACHE.lock().unwrap().get(&kind) {
        return *cached as *const usize;
    }
    let original = call_original!(kind);
    let update: Vec<(usize, usize)> = match WEAPON_UPDATES.lock().unwrap().get(&kind) {
        Some(update) => update.iter().map(|(s, f)| (*s, *f)).collect(),
        None => return original,
    };
    let Some(owner_kind) = clone_owner_kind(kind) else {
        return original;
    };
    if original.is_null() {
        return original;
    }

    let vptr = *(original as *const *const usize);
    let mut slots = [0usize; WEAPON_VTABLE_SLOTS];
    std::ptr::copy_nonoverlapping(vptr, slots.as_mut_ptr(), WEAPON_VTABLE_SLOTS);
    for (slot, func) in update {
        if let Some(thunk) = make_thunk(func, slots[slot], MODE_WEAPON, owner_kind) {
            slots[slot] = thunk;
        }
    }
    let patched = Box::leak(Box::new(WeaponVtable { vptr: std::ptr::null(), padding: [0; 2], slots }));
    patched.vptr = patched.slots.as_ptr();
    let addr = patched as *const WeaponVtable as usize;
    WEAPON_VTABLE_CACHE.lock().unwrap().insert(kind, addr);
    addr as *const usize
}

#[skyline::main(name = "smashline2_compat")]
pub fn main() {
    skyline::install_hooks!(get_fighter_vtable, get_weapon_vtable);
}
