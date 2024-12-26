pub unsafe trait FnOnceThunk<B: Copy> {
    const THUNK_TEMPLATE_ONCE: B;
}

pub unsafe trait FnMutThunk<B: Copy>: FnOnceThunk<B> {
    const THUNK_TEMPLATE_MUT: B;
}

pub unsafe trait FnThunk<B: Copy>: FnMutThunk<B> {
    const THUNK_TEMPLATE: B;
}

macro_rules! thunk_impl_triple {
    ($cconv:ty, $cconv_lit:literal, ($($id_tys: ident,)*) ($($args:ident: $tys:ty,)*)) => {
        unsafe impl<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>
            $crate::thunk::FnOnceThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_ONCE: unsafe extern $cconv_lit fn($($tys,)*) -> R = {
                unsafe extern $cconv_lit fn thunk<F: FnOnce($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *mut F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    closure_ptr.read()($($args),*)
                }
                thunk::<F, R, $($tys),*>
            };
        }
        unsafe impl<F: FnMut($($tys),*) -> R, R, $($id_tys),*>
            $crate::thunk::FnMutThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE_MUT: unsafe extern $cconv_lit fn($($tys,)*) -> R = {
                unsafe extern $cconv_lit fn thunk<F: FnMut($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *mut F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    (&mut *closure_ptr)($($args),*)
                }
                thunk::<F, R, $($tys),*>
            };
        }
        unsafe impl<F: Fn($($tys),*) -> R, R, $($id_tys),*>
            $crate::thunk::FnThunk<unsafe extern $cconv_lit fn($($tys,)*) -> R> for ($cconv, F)
        {
            const THUNK_TEMPLATE: unsafe extern $cconv_lit fn($($tys,)*) -> R = {
                unsafe extern $cconv_lit fn thunk<F: Fn($($tys),*) -> R, R, $($id_tys),*>($($args: $tys),*) -> R {
                    let closure_ptr: *const F;
                    $crate::arch::_thunk_asm!(closure_ptr);
                    (&*closure_ptr)($($args),*)
                }
                thunk::<F, R, $($tys),*>
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
