//! Raw declarations matching Hermes static_h.h at the pinned compiler revision.
use std::ffi::{c_char, c_void};

#[repr(C)]
pub struct Runtime {
    _opaque: [u8; 0],
}
#[repr(C)]
pub struct Unit {
    _opaque: [u8; 0],
}
pub type UnitCreator = unsafe extern "C" fn() -> *mut Unit;

unsafe extern "C" {
    pub fn _sh_init_with_error(
        argc: i32,
        argv: *mut *mut c_char,
        error: *mut *mut c_char,
    ) -> *mut Runtime;
    pub fn _sh_done(runtime: *mut Runtime);
    pub fn _sh_initialize_units(runtime: *mut Runtime, count: u32, ...) -> bool;
    pub fn sh_export_youtubei_host() -> *mut Unit;
    pub fn sh_export_youtubei_youtube() -> *mut Unit;
    pub fn sh_export_youtubei_driver() -> *mut Unit;
    pub fn free(pointer: *mut c_void);
}
