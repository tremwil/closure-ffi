// //! Defines calling convention marker types for use with `BareFn` variants.
// #![allow(clippy::incompatible_msrv)]

macro_rules! cc_fn_ptr_impl {
    (
        $num_args: tt,
        ($($id_tys:ident,)*),
        ($($tuple_idx:tt,)*),
        ($($args:ident: $tys:ty,)*),
        $cconv:ty,
        $cconv_lit:literal
    ) => {
        #[doc(hidden)]
        unsafe impl<R $(,$id_tys)*> $crate::traits::CallWith<($($tys,)*)> for
            unsafe extern $cconv_lit fn($($tys,)*) -> R
        {
            type Ret = R;

            #[allow(unused_variables)]
            #[inline(always)]
            unsafe fn call(self, args: ($($tys,)*)) -> Self::Ret {
                unsafe {
                    (self)($(args.$tuple_idx,)*)
                }
            }
        }

        unsafe impl<R $(,$id_tys)*> $crate::traits::FnPtr for
            unsafe extern $cconv_lit fn($($tys,)*) -> R
        {
            type CC = $cconv;

            #[inline(always)]
            unsafe fn from_ptr(ptr: *const ()) -> Self {
                unsafe { core::mem::transmute_copy(&ptr) }
            }

            #[inline(always)]
            fn to_ptr(self) -> *const () {
                self as *const ()
            }
        }
    };
}

// macro_rules! cc_thunk_impl_triple {
//     (
//         $cconv:ty,
//         $cconv_lit:literal,
//         $extra_idx: tt,
//         ($($id_tys: ident,)*),
//         ($($tuple_idx: tt,)*),
//         ($($args:ident: $tys:ty,)*)
//     ) => {
//         #[doc(hidden)]
//         unsafe impl<R, $($id_tys),*> $crate::traits::FnPtr for unsafe extern $cconv_lit
// fn($($tys,)*) -> R {             type CC = $cconv;
//             type Args<'a, 'b, 'c> = ($($tys,)*);
//             type Ret<'a, 'b, 'c> = R;

//             #[allow(unused_variables)]
//             #[inline(always)]
//             unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b,
// 'c>             {
//                 unsafe { (self)($(args.$tuple_idx,)*) }
//             }

//             #[inline(always)]
//             unsafe fn from_ptr(ptr: *const ()) -> Self {
//                 unsafe { core::mem::transmute_copy(&ptr) }
//             }

//             #[inline(always)]
//             fn to_ptr(self) -> *const () {
//                 self as *const _
//             }

//             #[inline(always)]
//             fn make_once_thunk<F>(fun: F) -> impl $crate::traits::FnOnceThunk<Self>
//             where
//                 F: for<'a, 'b, 'c> $crate::traits::PackedFnOnce<'a, 'b, 'c, Self>
//             {
//                 (Self::CC::default(), move |$($args,)*| fun(($($args,)*)))
//             }

//             #[inline(always)]
//             fn make_mut_thunk<F>(mut fun: F) -> impl $crate::traits::FnMutThunk<Self>
//             where
//                 F: for<'a, 'b, 'c> $crate::traits::PackedFnMut<'a, 'b, 'c, Self>
//             {
//                 (Self::CC::default(), move |$($args,)*| fun(($($args,)*)))
//             }

//             #[inline(always)]
//             fn make_thunk<F>(fun: F) -> impl $crate::traits::FnThunk<Self>
//             where
//                 F: for<'a, 'b, 'c> $crate::traits::PackedFn<'a, 'b, 'c, Self>
//             {
//                 (Self::CC::default(), move |$($args,)*| fun(($($args,)*)))
//             }
//         }

//         #[doc(hidden)]
//         unsafe impl<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>
//             $crate::traits::FnOnceThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv,
// F)         {
//             const THUNK_TEMPLATE_ONCE: *const u8 = {
//                 #[cfg_attr(feature = "coverage", coverage(off))]
//                 #[allow(clippy::too_many_arguments)]
//                 unsafe extern $cconv_lit fn thunk<F: FnOnce($($tys),*) -> R, R,
// $($id_tys),*>($($args: $tys),*) -> R {                     if const { core::mem::size_of::<F>()
// == 0 } {                         let fun: F = unsafe { core::mem::zeroed() };
//                         fun($($args),*)
//                     }
//                     else {
//                         let closure_ptr: *mut F;
//                         $crate::arch::_thunk_asm!(closure_ptr);
//                         $crate::arch::_invoke(|| closure_ptr.read()($($args),*))
//                     }
//                 }
//                 thunk::<F, R, $($tys),*> as *const u8
//             };

//             #[allow(unused_variables)]
//             #[inline(always)]
//             unsafe fn call_once<'a, 'b, 'c>(self, args: ($($tys,)*)) ->
//                 <unsafe extern $cconv_lit fn($($tys,)*) -> R as $crate::traits::FnPtr>::Ret<'a,
// 'b, 'c>             {
//                 (self.1)($(args.$tuple_idx,)*)
//             }
//         }

//         #[doc(hidden)]
//         unsafe impl<F: FnMut($($tys),*) -> R, R, $($id_tys),*>
//             $crate::traits::FnMutThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv,
// F)         {
//             const THUNK_TEMPLATE_MUT: *const u8 = {
//                 #[cfg_attr(feature = "coverage", coverage(off))]
//                 #[allow(clippy::too_many_arguments)]
//                 unsafe extern $cconv_lit fn thunk<F: FnMut($($tys),*) -> R, R,
// $($id_tys),*>($($args: $tys),*) -> R {                     if const { core::mem::size_of::<F>()
// == 0 } {                         let fun: &mut F = unsafe { &mut *core::ptr::dangling_mut() };
//                         fun($($args),*)
//                     }
//                     else {
//                         let closure_ptr: *mut F;
//                         $crate::arch::_thunk_asm!(closure_ptr);
//                         $crate::arch::_invoke(|| (&mut *closure_ptr)($($args),*))
//                     }
//                 }
//                 thunk::<F, R, $($tys),*> as *const u8
//             };

//             #[allow(unused_variables)]
//             #[inline(always)]
//             unsafe fn call_mut<'a, 'b, 'c>(&mut self, args: ($($tys,)*)) ->
//                 <unsafe extern $cconv_lit fn($($tys,)*) -> R as $crate::traits::FnPtr>::Ret<'a,
// 'b, 'c>             {
//                 (self.1)($(args.$tuple_idx,)*)
//             }

//         }

//         #[doc(hidden)]
//         unsafe impl<F: Fn($($tys),*) -> R, R, $($id_tys),*>
//             $crate::traits::FnThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
//         {
//             const THUNK_TEMPLATE: *const u8 = {
//                 #[cfg_attr(feature = "coverage", coverage(off))]
//                 #[allow(clippy::too_many_arguments)]
//                 unsafe extern $cconv_lit fn thunk<F: Fn($($tys),*) -> R, R,
// $($id_tys),*>($($args: $tys),*) -> R {                     if const { core::mem::size_of::<F>()
// == 0 } {                         let fun: &F = unsafe { &*core::ptr::dangling_mut() };
//                         fun($($args),*)
//                     }
//                     else {
//                         let closure_ptr: *const F;
//                         $crate::arch::_thunk_asm!(closure_ptr);
//                         $crate::arch::_invoke(|| (&*closure_ptr)($($args),*))
//                     }
//                 }
//                 thunk::<F, R, $($tys),*> as *const u8
//             };

//             #[allow(unused_variables)]
//             #[inline(always)]
//             unsafe fn call<'a, 'b, 'c>(&self, args: ($($tys,)*)) ->
//                 <unsafe extern $cconv_lit fn($($tys,)*) -> R as $crate::traits::FnPtr>::Ret<'a,
// 'b, 'c>             {
//                 (self.1)($(args.$tuple_idx,)*)
//             }
//         }
//     };
// }

macro_rules! cc_impl {
    ($ty_name:tt, $lit_name:literal $(,$cfg:meta)?) => {
        #[doc = "Marker type representing the"]
        #[doc = $lit_name]
        #[doc = "calling convention."]
        #[derive(Debug, Clone, Copy, Default)]
        $(#[cfg(any($cfg, doc))])?
        pub struct $ty_name;
        $(#[cfg($cfg)])?
        $crate::macros::variadic_generic!(cc_fn_ptr_impl!($ty_name, $lit_name));
    };
}

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

// #[rustversion::since(1.99)]
// macro_rules! cc_thunk_impl_triple_variadic {
//     (
//         $cconv:ty,
//         $cconv_lit:literal,
//         $extra_idx: tt,
//         ($($id_tys: ident,)*),
//         ($($tuple_idx: tt,)*),
//         ($($args:ident: $tys:ty,)*)
//     ) => {
//         #[doc(hidden)]
//         unsafe impl<R, $($id_tys),*> $crate::traits::FnPtr for unsafe extern $cconv_lit
// fn($($tys,)* ...) -> R {             type CC = $cconv;
//             type Args<'a, 'b, 'c> = ($($tys,)* core::ffi::VaList<'a>,);
//             type Ret<'a, 'b, 'c> = R;

//             #[allow(unused_variables)]
//             #[inline(always)]
//             unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b,
// 'c>             {
//                 const {
//                     panic!("FnPtr::call is not supported on C variadics due to a language
// limitations")                 }
//             }

//             #[inline(always)]
//             unsafe fn from_ptr(ptr: *const ()) -> Self {
//                 unsafe { core::mem::transmute_copy(&ptr) }
//             }

//             #[inline(always)]
//             fn to_ptr(self) -> *const () {
//                 self as *const _
//             }

//             #[inline(always)]
//             fn make_once_thunk<F>(fun: F) -> impl $crate::traits::FnOnceThunk<Self>
//             where
//                 F: for<'a, 'b, 'c> $crate::traits::PackedFnOnce<'a, 'b, 'c, Self>
//             {
//                 // needed to create a HRTB closure
//                 #[inline(always)]
//                 fn coerce<R, $($id_tys,)* F>(fun: F) -> F
//                 where F: for<'va> FnOnce($($tys,)* core::ffi::VaList<'va>) -> R {
//                     fun
//                 }
//                 let coerced = coerce(move |$($args,)* va| fun(($($args,)* va,)));
//                 (Self::CC::default(), coerced)
//             }

//             #[inline(always)]
//             fn make_mut_thunk<F>(mut fun: F) -> impl $crate::traits::FnMutThunk<Self>
//             where
//                 F: for<'a, 'b, 'c> $crate::traits::PackedFnMut<'a, 'b, 'c, Self>
//             {
//                 #[inline(always)]
//                 fn coerce<R, $($id_tys,)* F>(fun: F) -> F
//                 where F: for<'va> FnMut($($tys,)* core::ffi::VaList<'va>) -> R {
//                     fun
//                 }
//                 let coerced = coerce(move |$($args,)* va| fun(($($args,)* va,)));
//                 (Self::CC::default(), coerced)
//             }

//             #[inline(always)]
//             fn make_thunk<F>(fun: F) -> impl $crate::traits::FnThunk<Self>
//             where
//                 F: for<'a, 'b, 'c> $crate::traits::PackedFn<'a, 'b, 'c, Self>
//             {
//                 #[inline(always)]
//                 fn coerce<R, $($id_tys,)* F>(fun: F) -> F
//                 where F: for<'va> Fn($($tys,)* core::ffi::VaList<'va>) -> R {
//                     fun
//                 }
//                 let coerced = coerce(move |$($args,)* va| fun(($($args,)* va,)));
//                 (Self::CC::default(), coerced)
//             }
//         }

//         #[doc(hidden)]
//         unsafe impl<F: for<'va> FnOnce($($tys,)* core::ffi::VaList<'va>) -> R, R, $($id_tys),*>
//             $crate::traits::FnOnceThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for
// ($cconv, F)         {
//             const THUNK_TEMPLATE_ONCE: *const u8 = {
//                 #[cfg_attr(feature = "coverage", coverage(off))]
//                 unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args:
// ...) -> R                 where
//                     F: for<'va> FnOnce($($tys,)* core::ffi::VaList<'va>) -> R
//                 {
//                     if const { core::mem::size_of::<F>() == 0 } {
//                         let fun: F = unsafe { core::mem::zeroed() };
//                         fun($($args,)* va_args)
//                     }
//                     else {
//                         let closure_ptr: *mut F;
//                         $crate::arch::_thunk_asm!(closure_ptr);
//                         $crate::arch::_invoke(|| closure_ptr.read()($($args,)* va_args))
//                     }
//                 }
//                 thunk::<F, R, $($tys),*> as *const u8
//             };

//             #[allow(unused_variables)]
//             #[inline(always)]
//             unsafe fn call_once<'a, 'b, 'c>(
//                 self,
//                 args: <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as
// $crate::traits::FnPtr>::Args<'a, 'b, 'c>             ) ->
//                 <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as
// $crate::traits::FnPtr>::Ret<'a, 'b, 'c>             {
//                 (self.1)($(args.$tuple_idx,)* args.$extra_idx)
//             }
//         }

//         #[doc(hidden)]
//         unsafe impl<F: for<'va> FnMut($($tys,)* core::ffi::VaList<'va>) -> R, R, $($id_tys),*>
//             $crate::traits::FnMutThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for
// ($cconv, F)         {
//             const THUNK_TEMPLATE_MUT: *const u8 = {
//                 #[cfg_attr(feature = "coverage", coverage(off))]
//                 unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args:
// ...) -> R                 where
//                     F: for<'va> FnMut($($tys,)* core::ffi::VaList<'va>) -> R
//                 {
//                     if const { core::mem::size_of::<F>() == 0 } {
//                         let fun: &mut F = unsafe { &mut *core::ptr::dangling_mut() };
//                         fun($($args,)* va_args)
//                     }
//                     else {
//                         let closure_ptr: *mut F;
//                         $crate::arch::_thunk_asm!(closure_ptr);
//                         $crate::arch::_invoke(|| (&mut *closure_ptr)($($args,)* va_args))
//                     }
//                 }
//                 thunk::<F, R, $($tys),*> as *const u8
//             };

//             #[allow(unused_variables)]
//             #[inline(always)]
//             unsafe fn call_mut<'a, 'b, 'c>(
//                 &mut self,
//                 args: <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as
// $crate::traits::FnPtr>::Args<'a, 'b, 'c>             ) ->
//                 <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as
// $crate::traits::FnPtr>::Ret<'a, 'b, 'c>             {
//                 (self.1)($(args.$tuple_idx,)* args.$extra_idx)
//             }
//         }

//         #[doc(hidden)]
//         unsafe impl<F: for<'va> Fn($($tys,)* core::ffi::VaList<'va>) -> R, R, $($id_tys),*>
//             $crate::traits::FnThunk<unsafe extern $cconv_lit fn($($tys,)* ...) -> R> for ($cconv,
// F)         {
//             const THUNK_TEMPLATE: *const u8 = {
//                 #[cfg_attr(feature = "coverage", coverage(off))]
//                 unsafe extern $cconv_lit fn thunk<F, R, $($id_tys),*>($($args: $tys,)* va_args:
// ...) -> R                 where
//                     F: for<'va> Fn($($tys,)* core::ffi::VaList<'va>) -> R
//                 {
//                     if const { core::mem::size_of::<F>() == 0 } {
//                         let fun: &F = unsafe { &*core::ptr::dangling_mut() };
//                         fun($($args,)* va_args)
//                     }
//                     else {
//                         let closure_ptr: *const F;
//                         $crate::arch::_thunk_asm!(closure_ptr);
//                         $crate::arch::_invoke(|| (&*closure_ptr)($($args,)* va_args))
//                     }
//                 }
//                 thunk::<F, R, $($tys),*> as *const u8
//             };

//             #[allow(unused_variables)]
//             #[inline(always)]
//             unsafe fn call<'a, 'b, 'c>(
//                 &self,
//                 args: <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as
// $crate::traits::FnPtr>::Args<'a, 'b, 'c>             ) ->
//                 <unsafe extern $cconv_lit fn($($tys,)* ...) -> R as
// $crate::traits::FnPtr>::Ret<'a, 'b, 'c>             {
//                 (self.1)($(args.$tuple_idx,)* args.$extra_idx)
//             }
//         }
//     };
// }

// /// Marker type representing the C variadic calling convention.
// ///
// /// This is a separate marker type to enable richer type inference.
// #[rustversion::since(1.99)]
// #[derive(Debug, Clone, Copy, Default)]
// pub struct Variadic;
// #[rustversion::since(1.99)]
// cc_trait_impl!(Variadic, "C", cc_thunk_impl_triple_variadic);
