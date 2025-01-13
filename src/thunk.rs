#[allow(clippy::missing_safety_doc)]
pub unsafe trait FnOnceThunk<CC, Bare: Copy> {
    const THUNK_TEMPLATE_ONCE: *const u8;
}

#[allow(clippy::missing_safety_doc)]
pub unsafe trait FnMutThunk<CC, Bare: Copy>: FnOnceThunk<CC, Bare> {
    const THUNK_TEMPLATE_MUT: *const u8;
}

#[allow(clippy::missing_safety_doc)]
pub unsafe trait FnThunk<CC, Bare: Copy>: FnMutThunk<CC, Bare> {
    const THUNK_TEMPLATE: *const u8;
}

macro_rules! thunk_impl_triple {
    ($cconv:ty, $cconv_lit:literal, ($($id_tys: ident,)*) ($($args:ident: $tys:ty,)*)) => {
        unsafe impl<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>
            $crate::thunk::FnOnceThunk<$cconv, unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_ONCE: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *mut F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    closure_ptr.read()($($args),*)
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
        unsafe impl<F: FnMut($($tys),*) -> R, R, $($id_tys),*>
            $crate::thunk::FnMutThunk<$cconv, unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_MUT: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F: FnMut($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *mut F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    (&mut *closure_ptr)($($args),*)
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
        unsafe impl<F: Fn($($tys),*) -> R, R, $($id_tys),*>
            $crate::thunk::FnThunk<$cconv, unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE: *const u8 = {
                unsafe extern $cconv_lit fn thunk<F: Fn($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *const F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    (&*closure_ptr)($($args),*)
                }
                thunk::<F, R, $($tys),*> as *const u8
            };
        }
    };
}

macro_rules! cc_thunk_impl {
    ($cconv:ty, $cconv_lit:literal) => {
        // Support functions of up to 12 elements, like most traits on tuples
        ::seq_macro::seq!(M in 0..=12 {
            #(
                ::seq_macro::seq!(N in 0..M {
                    $crate::thunk::thunk_impl_triple!($cconv, $cconv_lit, (#(T~N,)*) (#(a~N: T~N,)*));
                });
            )*
        });
    };
}

pub(crate) use cc_thunk_impl;
pub(crate) use thunk_impl_triple;
