
#![feature(globs, lang_items)]
#![no_std]
#![no_main]

extern crate core;

use core::prelude::*;

#[lang = "begin_unwind"]
pub extern "C" fn rust_begin_unwind(_: &core::fmt::Arguments, _: &'static str, _: uint) -> ! { loop {} }
#[lang = "stack_exhausted"]
pub extern "C" fn rust_stack_exhausted() {}
#[lang = "eh_personality"]
fn rust_eh_personality() {}
#[no_mangle]
pub extern "C" fn __morestack() {}
#[no_mangle]
pub extern "C" fn __powisf2(mut a: f32, mut b: i32) -> f32 {
    let recip = b < 0;
    let mut r = 1.;
    loop {
        if b & 1 != 0 { r *= a }
        b /= 2;
        if b == 0 { break }
        a *= a;
    }
    if recip { r.recip() } else { r }
}
#[no_mangle]
pub extern "C" fn __powidf2(mut a: f64, mut b: i32) -> f64 {
    let recip = b < 0;
    let mut r = 1.;
    loop {
        if b & 1 != 0 { r *= a }
        b /= 2;
        if b == 0 { break }
        a *= a;
    }
    if recip { r.recip() } else { r }
}
#[no_mangle]
pub extern "C" fn main(_argc: int, _argv: *const *const u8) -> int {
    mymain();
    0
}

fn mymain() {
    display("Hello World!\0");
}
fn display(s: &str) {
    extern "system" {
        fn MessageBoxA(hWnd: u32, lpText: *const u8, lpCaption: *const u8, uType: u32) -> i32;
    }
    let p = s.as_ptr();
    unsafe {
        MessageBoxA(0, p, p, 0);
    }
}
