#![cfg(all(feature = "default_jit_alloc", feature = "c_variadic"))]
#![feature(c_variadic)]

use std::ffi::{CStr, VaList, VaListImpl};

use closure_ffi::BareFnMut;

#[cfg(not(all(target_arch = "x86", target_os = "windows")))]
unsafe extern "C" {
    fn vsprintf(buf: *mut u8, fmt: *const u8, va: VaList);
}

#[cfg(all(target_arch = "x86", target_os = "windows"))]
unsafe extern "stdcall" {
    fn vsprintf(buf: *mut u8, fmt: *const u8, va: VaList);
}

#[test]
fn test_variadic() {
    let mut buf = [0u8; 128];
    let fmt = b"dec = %d, hex = %llX, chr = %c, pi = %.2f\0";

    let bare_fn = BareFnMut::new_variadic(|mut va: VaListImpl| unsafe {
        vsprintf(buf.as_mut_ptr(), fmt.as_ptr(), va.as_va_list());
    });

    unsafe { bare_fn.bare()(42, 0xDEADBEEF123u64, '?', core::f64::consts::PI) }
    drop(bare_fn);

    let formatted = CStr::from_bytes_until_nul(&buf).unwrap().to_string_lossy();
    assert_eq!(formatted, "dec = 42, hex = DEADBEEF123, chr = ?, pi = 3.14");
}
