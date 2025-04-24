//! Defines calling convention marker types for use with `BareFn` variants.

#[cfg(feature = "proc_macros")]
#[doc(inline)]
pub use closure_ffi_proc_macros::hrtb_cc as hrtb;

macro_rules! cc_impl {
    ($ty_name:ident, $lit_name:literal $(,$cfg:meta)?) => {
        #[doc = "Marker type representing the"]
        #[doc = $lit_name]
        #[doc = "calling convention."]
        #[derive(Debug, Clone, Copy, Default)]
        $(#[cfg(any($cfg, doc))])?
        pub struct $ty_name;
        $(#[cfg($cfg)])?
        $crate::thunk::cc_thunk_impl!($ty_name, $lit_name);
    };
}

cc_impl!(C, "C");

cc_impl!(System, "system");

cc_impl!(Sysv64, "sysv64", all(not(windows), target_arch = "x86_64"));

cc_impl!(Aapcs, "aapcs", target_arch = "arm");

cc_impl!(
    Fastcall,
    "fastcall",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);

cc_impl!(
    Stdcall,
    "stdcall",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);

cc_impl!(
    Cdecl,
    "cdecl",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);

cc_impl!(Thiscall, "thiscall", all(windows, target_arch = "x86"));

cc_impl!(Win64, "win64", all(windows, target_arch = "x86_64"));
