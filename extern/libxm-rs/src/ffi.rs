#![allow(non_camel_case_types, missing_copy_implementations)]
extern crate libc;

use libc::{c_char, c_int, c_float, size_t};
use libc::{uint8_t, uint16_t, uint32_t, uint64_t};

pub enum xm_context {}
pub type xm_context_t = xm_context;

extern "C" {
    pub fn xm_create_context_safe(context: *mut *mut xm_context_t, moddata: *const c_char, moddata_length: size_t, rate: uint32_t) -> c_int;
    pub fn xm_free_context(context: *mut xm_context_t);
    pub fn xm_generate_samples(context: *mut xm_context_t, output: *mut c_float, numsamples: size_t);
    pub fn xm_set_max_loop_count(context: *mut xm_context_t, loopcnt: uint8_t);
    pub fn xm_get_loop_count(context: *mut xm_context_t) -> uint8_t;
    pub fn xm_get_module_name(context: *mut xm_context_t) -> *const c_char;
    pub fn xm_get_tracker_name(context: *mut xm_context_t) -> *const c_char;
    pub fn xm_get_number_of_channels(context: *mut xm_context_t) -> uint16_t;
    pub fn xm_get_module_length(context: *mut xm_context_t) -> uint16_t;
    pub fn xm_get_number_of_patterns(context: *mut xm_context_t) -> uint16_t;

    pub fn xm_get_number_of_rows(context: *mut xm_context_t, pattern: uint16_t) -> uint16_t;
    pub fn xm_get_number_of_instruments(context: *mut xm_context_t) -> uint16_t;
    pub fn xm_get_number_of_samples(context: *mut xm_context_t, instrument: uint16_t) -> uint16_t;

    pub fn xm_get_playing_speed(context: *mut xm_context_t, bpm: *mut uint16_t, tempo: *mut uint16_t);
    pub fn xm_get_position(context: *mut xm_context_t, pattern_index: *mut uint8_t, pattern: *mut uint8_t, row: *mut uint8_t, samples: *mut uint64_t);
    pub fn xm_get_latest_trigger_of_instrument(context: *mut xm_context_t, instrument: uint16_t) -> uint64_t;
    pub fn xm_get_latest_trigger_of_sample(context: *mut xm_context_t, instr: uint16_t, sample: uint16_t) -> uint64_t;
    pub fn xm_get_latest_trigger_of_channel(context: *mut xm_context_t, channel: uint16_t) -> uint64_t;
}
