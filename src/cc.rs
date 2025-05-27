//! Defines calling convention marker types for use with `BareFn` variants.

macro_rules! cc_thunk_impl_triple {
    ($cconv:ty, $cconv_lit:literal, ($($id_tys: ident,)*), ($($tuple_idx: tt,)*), ($($args:ident: $tys:ty,)*)) => {
        impl<R, $($id_tys),*> $crate::traits::FnPtr for unsafe extern $cconv_lit fn($($tys,)*) -> R {
            type CC = $cconv;
            type Args<'a, 'b, 'c> = ($($tys,)*);
            type Ret<'a, 'b, 'c> = R;

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b, 'c> {
                unsafe { (self)($(args.$tuple_idx,)*) }
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
    // Case 1: Non-empty ident lists
    (
        $cconv:ty,
        $cconv_lit:literal,
        [$head_id_ty:ident, $($tail_id_tys:ident,)*] ($($id_tys:ident,)*),
        [$head_tuple_idx:tt, $($tail_tuple_idx:tt,)*] ($($tuple_idx:tt,)*),
        [$head_arg:ident: $head_ty:ty, $($tail_args:ident: $tail_tys:ty,)*] ($($args:ident: $tys:ty,)*)
    ) => {
        cc_thunk_impl_triple!($cconv, $cconv_lit, ($($id_tys,)*), ($($tuple_idx,)*), ($($args: $tys,)*));

        cc_trait_impl_recursive!(
            $cconv,
            $cconv_lit,
            [$($tail_id_tys,)*] ($($id_tys,)* $head_id_ty,),
            [$($tail_tuple_idx,)*] ($($tuple_idx,)* $head_tuple_idx,),
            [$($tail_args: $tail_tys,)*] ($($args: $tys,)* $head_arg: $head_ty,)
        );
    };

    // Case 2: Exhausted ident lists
    (
        $cconv:ty,
        $cconv_lit:literal,
        [] ($($id_tys:ident,)*),
        [] ($($tuple_idx:tt,)*),
        [] ($($args:ident: $tys:ty,)*)
    ) => {
        cc_thunk_impl_triple!($cconv, $cconv_lit, ($($id_tys,)*), ($($tuple_idx,)*), ($($args: $tys,)*));
    };
}

macro_rules! cc_trait_impl {
    ($cconv:ty, $cconv_lit:literal) => {
        cc_trait_impl_recursive!(
            $cconv,
            $cconv_lit,
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
        cc_trait_impl!($ty_name, $lit_name);
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
