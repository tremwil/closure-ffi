//! Defines calling convention marker types for use with `BareFn` variants.

macro_rules! cc_thunk_impl_triple {
    (
        $cconv:ty,
        $cconv_lit:literal,
        $extra_idx: tt,
        ($($id_tys: ident,)*),
        ($($tuple_idx: tt,)*),
        ($($args:ident: $tys:ty,)*)
    ) => {
        #[doc(hidden)]
        unsafe impl<R, $($id_tys),*> $crate::traits::FnPtr for unsafe extern $cconv_lit fn($($tys,)*) -> R {
            type CC = $cconv;
            type Args<'a, 'b, 'c> = ($($tys,)*);
            type Ret<'a, 'b, 'c> = R;

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b, 'c>
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

            #[inline(always)]
            fn make_once_thunk<F>(fun: F) -> impl $crate::traits::FnOnceThunk<Self>
            where
                F: for<'a, 'b, 'c> $crate::traits::PackedFnOnce<'a, 'b, 'c, Self>
            {
                (Self::CC::default(), move |$($args,)*| fun(($($args,)*)))
            }

            #[inline(always)]
            fn make_mut_thunk<F>(mut fun: F) -> impl $crate::traits::FnMutThunk<Self>
            where
                F: for<'a, 'b, 'c> $crate::traits::PackedFnMut<'a, 'b, 'c, Self>
            {
                (Self::CC::default(), move |$($args,)*| fun(($($args,)*)))
            }

            #[inline(always)]
            fn make_thunk<F>(fun: F) -> impl $crate::traits::FnThunk<Self>
            where
                F: for<'a, 'b, 'c> $crate::traits::PackedFn<'a, 'b, 'c, Self>
            {
                (Self::CC::default(), move |$($args,)*| fun(($($args,)*)))
            }
        }

        #[doc(hidden)]
        unsafe impl<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>
            $crate::traits::FnOnceThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_ONCE: *const u8 = {
                #[cfg_attr(feature = "coverage", coverage(off))]
                #[allow(clippy::too_many_arguments)]
                unsafe extern $cconv_lit fn thunk<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    if const { core::mem::size_of::<F>() == 0 } {
                        let fun: F = unsafe { core::mem::zeroed() };
                        fun($($args),*)
                    }
                    else {
                        let closure_ptr: *mut F;
                        $crate::arch::_thunk_asm!(closure_ptr);
                        $crate::arch::_invoke(|| closure_ptr.read()($($args),*))
                    }
                }
                thunk::<F, R, $($tys),*> as *const u8
            };

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call_once<'a, 'b, 'c>(self, args: ($($tys,)*)) ->
                <unsafe extern $cconv_lit fn($($tys,)*) -> R as $crate::traits::FnPtr>::Ret<'a, 'b, 'c>
            {
                (self.1)($(args.$tuple_idx,)*)
            }
        }

        #[doc(hidden)]
        unsafe impl<F: FnMut($($tys),*) -> R, R, $($id_tys),*>
            $crate::traits::FnMutThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_MUT: *const u8 = {
                #[cfg_attr(feature = "coverage", coverage(off))]
                #[allow(clippy::too_many_arguments)]
                unsafe extern $cconv_lit fn thunk<F: FnMut($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    if const { core::mem::size_of::<F>() == 0 } {
                        let fun: &mut F = unsafe { &mut *core::ptr::dangling_mut() };
                        fun($($args),*)
                    }
                    else {
                        let closure_ptr: *mut F;
                        $crate::arch::_thunk_asm!(closure_ptr);
                        $crate::arch::_invoke(|| (&mut *closure_ptr)($($args),*))
                    }
                }
                thunk::<F, R, $($tys),*> as *const u8
            };

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call_mut<'a, 'b, 'c>(&mut self, args: ($($tys,)*)) ->
                <unsafe extern $cconv_lit fn($($tys,)*) -> R as $crate::traits::FnPtr>::Ret<'a, 'b, 'c>
            {
                (self.1)($(args.$tuple_idx,)*)
            }

        }

        #[doc(hidden)]
        unsafe impl<F: Fn($($tys),*) -> R, R, $($id_tys),*>
            $crate::traits::FnThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE: *const u8 = {
                #[cfg_attr(feature = "coverage", coverage(off))]
                #[allow(clippy::too_many_arguments)]
                unsafe extern $cconv_lit fn thunk<F: Fn($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    if const { core::mem::size_of::<F>() == 0 } {
                        let fun: &F = unsafe { &*core::ptr::dangling_mut() };
                        fun($($args),*)
                    }
                    else {
                        let closure_ptr: *const F;
                        $crate::arch::_thunk_asm!(closure_ptr);
                        $crate::arch::_invoke(|| (&*closure_ptr)($($args),*))
                    }
                }
                thunk::<F, R, $($tys),*> as *const u8
            };

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call<'a, 'b, 'c>(&self, args: ($($tys,)*)) ->
                <unsafe extern $cconv_lit fn($($tys,)*) -> R as $crate::traits::FnPtr>::Ret<'a, 'b, 'c>
            {
                (self.1)($(args.$tuple_idx,)*)
            }
        }
    };
}

macro_rules! cc_trait_impl_recursive {
    // Case 1: Non-empty parameter lists
    (
        $cconv:ty,
        $cconv_lit:literal,
        $impl_macro:ident,
        [$head_extra_idx:tt, $($tail_extra_idx:tt,)*],
        [$head_id_ty:ident, $($tail_id_tys:ident,)*] ($($id_tys:ident,)*),
        [$head_tuple_idx:tt, $($tail_tuple_idx:tt,)*] ($($tuple_idx:tt,)*),
        [$head_arg:ident: $head_ty:ty, $($tail_args:ident: $tail_tys:ty,)*] ($($args:ident: $tys:ty,)*)
    ) => {
        $impl_macro!(
            $cconv,
            $cconv_lit,
            $head_extra_idx,
            ($($id_tys,)*),
            ($($tuple_idx,)*),
            ($($args: $tys,)*)
        );

        cc_trait_impl_recursive!(
            $cconv,
            $cconv_lit,
            $impl_macro,
            [$($tail_extra_idx,)*],
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
        [$extra_idx:tt,],
        [] ($($id_tys:ident,)*),
        [] ($($tuple_idx:tt,)*),
        [] ($($args:ident: $tys:ty,)*)
    ) => {
        $impl_macro!(
            $cconv,
            $cconv_lit,
            $extra_idx,
            ($($id_tys,)*),
            ($($tuple_idx,)*),
            ($($args: $tys,)*)
        );
    };
}

macro_rules! cc_trait_impl {
    ($cconv:ty, $cconv_lit:literal, $impl_macro:ident) => {
        cc_trait_impl_recursive!(
            $cconv,
            $cconv_lit,
            $impl_macro,
            [0,1,2,3,4,5,6,7,8,9,10,11,12,],
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

/// Marker type representing the Rust calling convention.
///
/// Note that since Rust has no stable ABI, it may change across compiler versions. Although
/// unlikely, it is even allowed to change between compiler invocations. Do not rely on "Rust" bare
/// functions from different binaries to be ABI compatible.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rust;
cc_trait_impl!(Rust, "Rust", cc_thunk_impl_triple);

cc_impl!(C, "C");
cc_impl!(CUnwind, "C-unwind");

cc_impl!(System, "system");
cc_impl!(SystemUnwind, "system-unwind");

cc_impl!(Efiapi, "efiapi");

cc_impl!(Sysv64, "sysv64", target_arch = "x86_64");
cc_impl!(Sysv64Unwind, "sysv64-unwind", target_arch = "x86_64");

cc_impl!(Win64, "win64", target_arch = "x86_64");
cc_impl!(Win64Unwind, "win64-unwind", target_arch = "x86_64");

cc_impl!(Aapcs, "aapcs", target_arch = "arm");
cc_impl!(AapcsUnwind, "aapcs-unwind", target_arch = "arm");

cc_impl!(Fastcall, "fastcall", target_arch = "x86");
cc_impl!(FastcallUnwind, "fastcall-unwind", target_arch = "x86");

cc_impl!(Stdcall, "stdcall", target_arch = "x86");
cc_impl!(StdcallUnwind, "stdcall-unwind", target_arch = "x86");

cc_impl!(Cdecl, "cdecl", target_arch = "x86");
cc_impl!(CdeclUnwind, "cdecl-unwind", target_arch = "x86");

cc_impl!(Thiscall, "thiscall", target_arch = "x86");
cc_impl!(ThiscallUnwind, "thiscall-unwind", target_arch = "x86");

#[cfg(feature = "c_variadic")]
macro_rules! cc_thunk_impl_triple_variadic {
    (
        $cconv:ty,
        $cconv_lit:literal,
        $extra_idx: tt,
        ($($id_tys: ident,)*),
        ($($tuple_idx: tt,)*),
        ($($args:ident: $tys:ty,)*)
    ) => {
        #[doc(hidden)]
        unsafe impl<R, $($id_tys),*> $crate::traits::FnPtr for unsafe extern $cconv_lit fn($($tys,)* ...) -> R {
            type CC = $cconv;
            type Args<'a, 'b, 'c> = ($($tys,)* core::ffi::VaList<'a>,);
            type Ret<'a, 'b, 'c> = R;

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b, 'c>
            {
                const {
                    panic!("FnPtr::call is not supported on C variadics due to a language limitations")
                }
            }

            #[inline(always)]
            unsafe fn from_ptr(ptr: *const ()) -> Self {
                unsafe { core::mem::transmute_copy(&ptr) }
            }

            #[inline(always)]
            fn to_ptr(self) -> *const () {
                self as *const _
            }

            #[inline(always)]
            fn make_once_thunk<F>(fun: F) -> impl $crate::traits::FnOnceThunk<Self>
            where
                F: for<'a, 'b, 'c> $crate::traits::PackedFnOnce<'a, 'b, 'c, Self>
            {
                // needed to create a HRTB closure
                #[inline(always)]
                fn coerce<R, $($id_tys,)* F>(fun: F) -> F
                where F: for<'va> FnOnce($($tys,)* core::ffi::VaList<'va>) -> R {
                    fun
                }
                let coerced = coerce(move |$($args,)* va| fun(($($args,)* va,)));
                (Self::CC::default(), coerced)
            }

            #[inline(always)]
            fn make_mut_thunk<F>(mut fun: F) -> impl $crate::traits::FnMutThunk<Self>
            where
                F: for<'a, 'b, 'c> $crate::traits::PackedFnMut<'a, 'b, 'c, Self>
            {
                #[inline(always)]
                fn coerce<R, $($id_tys,)* F>(fun: F) -> F
                where F: for<'va> FnMut($($tys,)* core::ffi::VaList<'va>) -> R {
                    fun
                }
                let coerced = coerce(move |$($args,)* va| fun(($($args,)* va,)));
                (Self::CC::default(), coerced)
            }

            #[inline(always)]
            fn make_thunk<F>(fun: F) -> impl $crate::traits::FnThunk<Self>
            where
                F: for<'a, 'b, 'c> $crate::traits::PackedFn<'a, 'b, 'c, Self>
            {
                #[inline(always)]
                fn coerce<R, $($id_tys,)* F>(fun: F) -> F
                where F: for<'va> Fn($($tys,)* core::ffi::VaList<'va>) -> R {
                    fun
                }
                let coerced = coerce(move |$($args,)* va| fun(($($args,)* va,)));
                (Self::CC::default(), coerced)
            }
        }

        #[doc(hidden)]
        unsafe impl<F: for<'va> FnOnce($($tys,)* core::ffi::VaList<'va>) -> R, R, $($id_tys),*>
            $crate::traits::FnOnceThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_ONCE: *const u8 = {
                #[cfg_attr(feature = "coverage", coverage(off))]
                unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args: ...) -> R
                where
                    F: for<'va> FnOnce($($tys,)* core::ffi::VaList<'va>) -> R
                {
                    if const { core::mem::size_of::<F>() == 0 } {
                        let fun: F = unsafe { core::mem::zeroed() };
                        fun($($args,)* va_args)
                    }
                    else {
                        let closure_ptr: *mut F;
                        $crate::arch::_thunk_asm!(closure_ptr);
                        $crate::arch::_invoke(|| closure_ptr.read()($($args,)* va_args))
                    }
                }
                thunk::<F, R, $($tys),*> as *const u8
            };

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call_once<'a, 'b, 'c>(
                self,
                args: <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as $crate::traits::FnPtr>::Args<'a, 'b, 'c>
            ) ->
                <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as $crate::traits::FnPtr>::Ret<'a, 'b, 'c>
            {
                (self.1)($(args.$tuple_idx,)* args.$extra_idx)
            }
        }

        #[doc(hidden)]
        unsafe impl<F: for<'va> FnMut($($tys,)* core::ffi::VaList<'va>) -> R, R, $($id_tys),*>
            $crate::traits::FnMutThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_MUT: *const u8 = {
                #[cfg_attr(feature = "coverage", coverage(off))]
                unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args: ...) -> R
                where
                    F: for<'va> FnMut($($tys,)* core::ffi::VaList<'va>) -> R
                {
                    if const { core::mem::size_of::<F>() == 0 } {
                        let fun: &mut F = unsafe { &mut *core::ptr::dangling_mut() };
                        fun($($args,)* va_args)
                    }
                    else {
                        let closure_ptr: *mut F;
                        $crate::arch::_thunk_asm!(closure_ptr);
                        $crate::arch::_invoke(|| (&mut *closure_ptr)($($args,)* va_args))
                    }
                }
                thunk::<F, R, $($tys),*> as *const u8
            };

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call_mut<'a, 'b, 'c>(
                &mut self,
                args: <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as $crate::traits::FnPtr>::Args<'a, 'b, 'c>
            ) ->
                <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as $crate::traits::FnPtr>::Ret<'a, 'b, 'c>
            {
                (self.1)($(args.$tuple_idx,)* args.$extra_idx)
            }
        }

        #[doc(hidden)]
        unsafe impl<F: for<'va> Fn($($tys,)* core::ffi::VaList<'va>) -> R, R, $($id_tys),*>
            $crate::traits::FnThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE: *const u8 = {
                #[cfg_attr(feature = "coverage", coverage(off))]
                unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args: ...) -> R
                where
                    F: for<'va> Fn($($tys,)* core::ffi::VaList<'va>) -> R
                {
                    if const { core::mem::size_of::<F>() == 0 } {
                        let fun: &F = unsafe { &*core::ptr::dangling_mut() };
                        fun($($args,)* va_args)
                    }
                    else {
                        let closure_ptr: *const F;
                        $crate::arch::_thunk_asm!(closure_ptr);
                        $crate::arch::_invoke(|| (&*closure_ptr)($($args,)* va_args))
                    }
                }
                thunk::<F, R, $($tys),*> as *const u8
            };

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call<'a, 'b, 'c>(
                &self,
                args: <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as $crate::traits::FnPtr>::Args<'a, 'b, 'c>
            ) ->
                <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as $crate::traits::FnPtr>::Ret<'a, 'b, 'c>
            {
                (self.1)($(args.$tuple_idx,)* args.$extra_idx)
            }
        }
    };
}

/// Marker type representing the C variadic calling convention.
///
/// This is a separate marker type to enable richer type inference.
#[derive(Debug, Clone, Copy, Default)]
#[cfg(any(doc, feature = "c_variadic"))]
pub struct Variadic;
#[cfg(feature = "c_variadic")]
cc_trait_impl!(Variadic, "C", cc_thunk_impl_triple_variadic);
