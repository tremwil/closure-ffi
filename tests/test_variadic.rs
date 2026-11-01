#[rustversion::since(1.99)]
mod slab_alloc;

#[rustversion::since(1.99)]
mod variadic {
    use std::ffi::{CStr, VaList};

    use closure_ffi::BareFnMut;

    use super::slab_alloc::SLAB;

    #[cfg_attr(target_env = "msvc", link(name = "legacy_stdio_definitions"))]
    unsafe extern "C" {
        fn vsprintf(buf: *mut u8, fmt: *const u8, va: VaList<'_>);
    }

    #[test]
    fn test_variadic() {
        let mut buf = [0u8; 128];
        let fmt = b"dec = %d, hex = %llX, chr = %c, pi = %.2f\0";

        let bare_fn = BareFnMut::new_variadic_in(
            |va: VaList| unsafe {
                vsprintf(buf.as_mut_ptr(), fmt.as_ptr(), va);
            },
            &SLAB,
        );

        unsafe { bare_fn.bare()(42, 0xDEADBEEF123u64, '?', core::f64::consts::PI) }
        drop(bare_fn);

        let formatted = CStr::from_bytes_until_nul(&buf).unwrap().to_string_lossy();
        assert_eq!(formatted, "dec = 42, hex = DEADBEEF123, chr = ?, pi = 3.14");
    }
}
