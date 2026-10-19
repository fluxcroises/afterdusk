//! Déclarations FFI de la couche `shim/afterdusk_raw.h`.
//!
//! Utiliser le module `raw` d'Afterdusk plutôt que ces fonctions directement.

#![allow(non_camel_case_types)]

use std::ffi::c_char;
use std::os::raw::c_int;

#[repr(C)]
pub struct ad_raw {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ad_raw_info {
    pub make: [c_char; 64],
    pub model: [c_char; 64],
    pub normalized_make: [c_char; 64],
    pub normalized_model: [c_char; 64],
    pub software: [c_char; 64],
    pub lens_make: [c_char; 128],
    pub lens: [c_char; 128],
    pub body_serial: [c_char; 64],
    pub artist: [c_char; 64],
    pub iso_speed: f32,
    pub shutter: f32,
    pub aperture: f32,
    pub focal_len: f32,
    pub focal_len_35mm: u16,
    pub timestamp: i64,
    pub raw_width: u16,
    pub raw_height: u16,
    pub width: u16,
    pub height: u16,
    pub flip: i32,
    pub filters: u32,
    pub colors: i32,
    pub dng_version: u32,
    pub gps_parsed: i32,
    pub gps_latitude: [f32; 3],
    pub gps_longitude: [f32; 3],
    pub gps_altitude: f32,
    pub gps_latref: c_char,
    pub gps_longref: c_char,
    pub gps_altref: c_char,
    pub thumb_count: i32,
}

pub const AD_IFD_EXIF: u32 = 0;
pub const AD_IFD_TIFF0: u32 = 1;
pub const AD_IFD_GPS: u32 = 4;

/// `LIBRAW_INTERNAL_THUMBNAIL_JPEG` dans la liste des aperçus.
pub const AD_INTERNAL_THUMB_JPEG: c_int = 4;

pub const AD_THUMB_JPEG: c_int = 1;
pub const AD_THUMB_BITMAP: c_int = 2;
pub const AD_THUMB_JPEGXL: c_int = 3;

unsafe extern "C" {
    pub fn ad_raw_open(utf8_path: *const c_char, error: *mut c_int) -> *mut ad_raw;
    pub fn ad_raw_close(raw: *mut ad_raw);
    pub fn ad_raw_get_info(raw: *const ad_raw) -> *const ad_raw_info;
    pub fn ad_raw_exif_count(raw: *const ad_raw) -> usize;
    pub fn ad_raw_exif_get(
        raw: *const ad_raw,
        index: usize,
        ifd: *mut u32,
        tag: *mut u32,
        value: *mut *const c_char,
    ) -> c_int;
    pub fn ad_raw_thumb_count(raw: *const ad_raw) -> c_int;
    pub fn ad_raw_thumb_item(
        raw: *const ad_raw,
        index: c_int,
        format: *mut c_int,
        width: *mut u16,
        height: *mut u16,
        length: *mut u32,
    ) -> c_int;
    pub fn ad_raw_thumbnail(
        raw: *mut ad_raw,
        index: c_int,
        data: *mut *const u8,
        size: *mut usize,
        format: *mut c_int,
        width: *mut u16,
        height: *mut u16,
        colors: *mut u16,
        bits: *mut u16,
    ) -> c_int;
    pub fn ad_raw_develop(
        raw: *mut ad_raw,
        half_size: c_int,
        data: *mut *const u16,
        width: *mut u16,
        height: *mut u16,
    ) -> c_int;
    pub fn ad_raw_strerror(error: c_int) -> *const c_char;
    pub fn ad_raw_version() -> *const c_char;
}
