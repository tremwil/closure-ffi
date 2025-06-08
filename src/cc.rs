//! Defines calling convention marker types for use with `BareFn` variants.

macro_rules! cc_thunk_impl_triple {
    ($cconv:ty, $cconv_lit:literal, ($($id_tys: ident,)*), ($($tuple_idx: tt,)*), ($($args:ident: $tys:ty,)*)) => {
        unsafe impl<R, $($id_tys),*> $crate::traits::FnPtr for unsafe extern $cconv_lit fn($($tys,)*) -> R {
            type CC = $cconv;
            type Args<'a, 'b, 'c> = ($($tys,)*) where Self: 'a + 'b + 'c;
            type Ret<'a, 'b, 'c> = R where Self: 'a + 'b + 'c;

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b, 'c>
                where Self: 'a + 'b + 'c
            {
                unsafe { (self)($(args.$tuple_idx,)*) }
            }

            #[inline(always)]
            unsafe fn from_ptr(ptr: *const ()) -> Self {
                unsafe { core::mem::transmute_copy(&ptr) }
            }

            #[inline(always)]
            fn to_ptr(self) -> *const () {
                self as *const _
            }
        }

        unsafe impl<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>
            $crate::traits::FnOnceThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_ONCE: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *mut F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    $crate::arch::_never_inline(|| closure_ptr.read()($($args),*))
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
        unsafe impl<F: FnMut($($tys),*) -> R, R, $($id_tys),*>
            $crate::traits::FnMutThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_MUT: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F: FnMut($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *mut F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    $crate::arch::_never_inline(|| (&mut *closure_ptr)($($args),*))
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
        unsafe impl<F: Fn($($tys),*) -> R, R, $($id_tys),*>
            $crate::traits::FnThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F: Fn($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *const F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    $crate::arch::_never_inline(|| (&*closure_ptr)($($args),*))
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
    };
}

macro_rules! cc_trait_impl_recursive {
    // Case 1: Non-empty parameter lists
    (
        $cconv:ty,
        $cconv_lit:literal,
        $impl_macro:ident,
        [$head_id_ty:ident, $($tail_id_tys:ident,)*] ($($id_tys:ident,)*),
        [$head_tuple_idx:tt, $($tail_tuple_idx:tt,)*] ($($tuple_idx:tt,)*),
        [$head_arg:ident: $head_ty:ty, $($tail_args:ident: $tail_tys:ty,)*] ($($args:ident: $tys:ty,)*)
    ) => {
        $impl_macro!($cconv, $cconv_lit, ($($id_tys,)*), ($($tuple_idx,)*), ($($args: $tys,)*));

        cc_trait_impl_recursive!(
            $cconv,
            $cconv_lit,
            $impl_macro,
            [$($tail_id_tys,)*] ($($id_tys,)* $head_id_ty,),
            [$($tail_tuple_idx,)*] ($($tuple_idx,)* $head_tuple_idx,),
            [$($tail_args: $tail_tys,)*] ($($args: $tys,)* $head_arg: $head_ty,)
        );
    };

    // Case 2: Exhausted parameter lists
    (
        $cconv:ty,
        $cconv_lit:literal,
        $impl_macro:ident,
        [] ($($id_tys:ident,)*),
        [] ($($tuple_idx:tt,)*),
        [] ($($args:ident: $tys:ty,)*)
    ) => {
        $impl_macro!($cconv, $cconv_lit, ($($id_tys,)*), ($($tuple_idx,)*), ($($args: $tys,)*));
    };
}

macro_rules! cc_trait_impl {
    ($cconv:ty, $cconv_lit:literal, $impl_macro:ident) => {
        cc_trait_impl_recursive!(
            $cconv,
            $cconv_lit,
            $impl_macro,
            [T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,](),
            [0,1,2,3,4,5,6,7,8,9,10,11,](),
            [a0:T0,a1:T1,a2:T2,a3:T3,a4:T4,a5:T5,a6:T6,a7:T7,a8:T8,a9:T9,a10:T10,a11:T11,]()
        );
    };
}

macro_rules! cc_impl {
    ($ty_name:tt, $lit_name:literal $(,$cfg:meta)?) => {
        #[doc = "Marker type representing the"]
        #[doc = $lit_name]
        #[doc = "calling convention."]
        #[derive(Debug, Clone, Copy, Default)]
        $(#[cfg(any($cfg, doc))])?
        pub struct $ty_name;
        $(#[cfg($cfg)])?
        cc_trait_impl!($ty_name, $lit_name, cc_thunk_impl_triple);
    };
}

cc_impl!(C, "C");
cc_impl!(CUnwind, "C-unwind");

cc_impl!(System, "system");
cc_impl!(SystemUnwind, "system-unwind");

cc_impl!(Sysv64, "sysv64", all(not(windows), target_arch = "x86_64"));
cc_impl!(
    Sysv64Unwind,
    "sysv64-unwind",
    all(not(windows), target_arch = "x86_64")
);

cc_impl!(Aapcs, "aapcs", target_arch = "arm");
cc_impl!(AapcsUnwind, "aapcs-unwind", target_arch = "arm");

cc_impl!(
    Fastcall,
    "fastcall",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);
cc_impl!(
    FastcallUnwind,
    "fastcall-unwind",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);

cc_impl!(
    Stdcall,
    "stdcall",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);
cc_impl!(
    StdcallUnwind,
    "stdcall-unwind",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);

cc_impl!(
    Cdecl,
    "cdecl",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);
cc_impl!(
    CdeclUnwind,
    "cdecl-unwind",
    all(windows, any(target_arch = "x86_64", target_arch = "x86"))
);

cc_impl!(Thiscall, "thiscall", all(windows, target_arch = "x86"));
cc_impl!(
    ThiscallUnwind,
    "thiscall-unwind",
    all(windows, target_arch = "x86")
);

cc_impl!(Win64, "win64", all(windows, target_arch = "x86_64"));
cc_impl!(
    Win64Unwind,
    "win64-unwind",
    all(windows, target_arch = "x86_64")
);

/// Marker type representing the variadic part of a C variadic function's arguments.
///
/// Because one cannot forward arguments from one to another C variadic function, this is a
/// zero-variant enum in order to make [`FnPtr::call`](crate::traits::FnPtr) impossible to call.
#[cfg(feature = "c_variadic")]
pub enum VarArgs {}

#[cfg(feature = "c_variadic")]
macro_rules! cc_thunk_impl_triple_variadic {
    ($cconv:ty, $cconv_lit:literal, ($($id_tys: ident,)*), ($($tuple_idx: tt,)*), ($($args:ident: $tys:ty,)*)) => {
        unsafe impl<R, $($id_tys),*> $crate::traits::FnPtr for unsafe extern $cconv_lit fn($($tys,)* ...) -> R {
            type CC = $cconv;
            type Args<'a, 'b, 'c> = ($($tys,)* VarArgs,) where Self: 'a + 'b + 'c;
            type Ret<'a, 'b, 'c> = R where Self: 'a + 'b + 'c;

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b, 'c>
                where Self: 'a + 'b + 'c
            {
                // SAFETY: `args` contains a zero-variant enum and as such this function is impossible to call.
                unsafe { core::hint::unreachable_unchecked() }
            }

            #[inline(always)]
            unsafe fn from_ptr(ptr: *const ()) -> Self {
                unsafe { core::mem::transmute_copy(&ptr) }
            }

            #[inline(always)]
            fn to_ptr(self) -> *const () {
                self as *const _
            }
        }

        unsafe impl<F: for<'va> FnOnce($($tys,)* core::ffi::VaListImpl<'va>) -> R, R, $($id_tys),*>
            $crate::traits::FnOnceThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_ONCE: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args: ...) -> R
                where
                    F: for<'va> FnOnce($($tys,)* core::ffi::VaListImpl<'va>) -> R
                {
                    let closure_ptr: *mut F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    $crate::arch::_never_inline(|| closure_ptr.read()($($args,)* va_args))
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
        unsafe impl<F: for<'va> FnMut($($tys,)* core::ffi::VaListImpl<'va>) -> R, R, $($id_tys),*>
            $crate::traits::FnMutThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_MUT: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args: ...) -> R
                where
                    F: for<'va> FnMut($($tys,)* core::ffi::VaListImpl<'va>) -> R
                {
                    let closure_ptr: *mut F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    $crate::arch::_never_inline(|| (&mut *closure_ptr)($($args,)* va_args))
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
        unsafe impl<F: for<'va> Fn($($tys,)* core::ffi::VaListImpl<'va>) -> R, R, $($id_tys),*>
            $crate::traits::FnThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args: ...) -> R
                where
                    F: for<'va> Fn($($tys,)* core::ffi::VaListImpl<'va>) -> R
                {
                    let closure_ptr: *const F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    $crate::arch::_never_inline(|| (&*closure_ptr)($($args,)* va_args))
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
    };
}

#[cfg(feature = "c_variadic")]
cc_trait_impl!(C, "C", cc_thunk_impl_triple_variadic);
