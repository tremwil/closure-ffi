macro_rules! cc_impl {
    ($ty_name:ident, $lit_name:literal) => {
        #[doc = "Marker type representing the"]
        #[doc = $lit_name]
        #[doc = "calling convention."]
        #[derive(Debug, Clone, Copy)]
        pub struct $ty_name;
        $crate::thunk::cc_thunk_impl!($ty_name, $lit_name);
    };
}

cc_impl!(C, "C");

cc_impl!(System, "system");

#[cfg(all(not(windows), target_arch = "x86_64"))]
cc_impl!(Sysv64, "sysv64");

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
cc_impl!(Aapcs, "aapcs");

#[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
cc_impl!(Fastcall, "fastcall");

#[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
cc_impl!(Stdcall, "stdcall");

#[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
cc_impl!(Cdecl, "cdecl");

#[cfg(all(windows, target_arch = "x86"))]
cc_impl!(Thiscall, "thiscall");

#[cfg(all(windows, target_arch = "x86_64"))]
cc_impl!(Win64, "win64");
