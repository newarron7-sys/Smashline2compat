//! Narrow compatibility shim for the installed One-Slot Effects resolver.
use smash::{app::{self, lua_bind::WorkModule}, lib::lua_const::*};
use std::{ffi::c_void, sync::atomic::{AtomicU32, AtomicUsize, Ordering}};

#[repr(C)]
#[derive(Clone, Copy)]
struct Resolution {
    found: u64,
    hash: u64,
}

type Resolve = unsafe extern "C" fn(u32, u64) -> Resolution;
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static RC_THREAD: AtomicUsize = AtomicUsize::new(0);
static RC_SLOT: AtomicU32 = AtomicU32::new(0);

const STOCK_STAMP: u64 = smash::hash40("sys_machstamp");
const RC_NAMES: [&str; 10] = [
    "kamui_red_rc", "kamui_blue_rc", "kamui_yellow_rc", "kamui_purple_rc",
    "kamui_red_rc_blast", "kamui_blue_rc_blast", "kamui_yellow_rc_blast",
    "kamui_purple_rc_blast", "rc_clock", "rc_clock_base",
];

pub(crate) unsafe fn with_rc_slot(slot: i32, callback: impl FnOnce()) {
    RC_SLOT.store(slot as u32, Ordering::Relaxed);
    RC_THREAD.store(skyline::nn::os::GetCurrentThread() as usize, Ordering::Release);
    callback();
    RC_THREAD.store(0, Ordering::Release);
}

unsafe extern "C" fn resolve(object: u32, hash: u64) -> Resolution {
    if hash == STOCK_STAMP {
        // Stock effect. Return None so the original caller uses the supplied hash.
        return Resolution { found: 0, hash };
    }
    if let Some(name) = RC_NAMES.iter().find(|name| smash::hash40(name) == hash) {
        let thread = RC_THREAD.load(Ordering::Acquire);
        let slot = if thread != 0 && thread == skyline::nn::os::GetCurrentThread() as usize {
            RC_SLOT.load(Ordering::Relaxed) as i32
        } else if object >> 28 == 0 && app::sv_battle_object::is_active(object) {
            let b = app::sv_battle_object::module_accessor(object);
            if b.is_null() || app::utility::get_kind(&mut *b) != *FIGHTER_KIND_CAPTAIN {
                -1
            } else {
                WorkModule::get_int(b, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR)
            }
        } else {
            -1
        };
        if crate::is_slot(slot) {
            return Resolution { found: 1, hash: crate::effect_slot_hash(slot, name).hash };
        }
    }
    let original = ORIGINAL.load(Ordering::Acquire);
    if original == 0 {
        return Resolution { found: 0, hash };
    }
    let call: Resolve = std::mem::transmute(original);
    call(object, hash)
}

pub(crate) unsafe fn install() {
    let mut symbol = 0usize;
    skyline::nn::ro::LookupSymbol(&mut symbol, b"make_effect\0".as_ptr());
    if symbol == 0 {
        return;
    }
    let base = symbol - 0x9314;
    let address = (base + 0x3224) as *const u32;
    let expected = [0xa9ba7bfd, 0xa9016ffc, 0xa90267fa, 0xa9035ff8, 0xa90457f6];
    if std::slice::from_raw_parts(address, expected.len()) != expected
        || *((base + 0x9328) as *const u32) != 0x97ffe7bf
    {
        skyline::println!("[Slayer] One-Slot Effects resolver differs; compatibility hook skipped");
        return;
    }
    let mut original: *mut c_void = std::ptr::null_mut();
    skyline::hooks::A64HookFunction(address.cast(), resolve as *const c_void, &mut original);
    ORIGINAL.store(original as usize, Ordering::Release);
    skyline::println!("[Slayer] One-Slot Effects compatibility hook installed");
}
