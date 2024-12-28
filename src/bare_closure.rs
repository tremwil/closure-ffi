use core::{marker::PhantomData, mem::ManuallyDrop, pin::Pin};

#[cfg(feature = "no_std")]
use alloc::Box;

use crate::{
    arch::{create_thunk, ThunkInfo},
    cc,
    jit_alloc::{self, JitAlloc, JitAllocError},
    thunk::{FnMutThunk, FnOnceThunk, FnThunk},
};

macro_rules! cc_shorthand {
    ($fn_name:ident, $trait_ident:ident, $cc_ty:ty, $cc_name:literal) => {
        #[doc = "Create a bare function thunk using the "]
        #[doc = $cc_name]
        #[doc = "calling convention for `fun`."]
        #[inline]
        pub fn $fn_name(fun: F) -> Self
        where
            ($cc_ty, F): $trait_ident<$cc_ty, B>,
        {
            Self::new(<$cc_ty>::default(), fun)
        }
    };
}

macro_rules! bare_closure {
    (
        $ty_name:ident, 
        $trait_ident:ident, 
        $thunk_template:ident, 
        $fn_trait_doc:literal, 
        $safety_doc:literal
    ) => {
        /// Wrapper around a
        #[doc = $fn_trait_doc]
        /// closure which exposes a bare function thunk that can invoke it without
        /// additional arguments.
        #[allow(dead_code)]
        pub struct $ty_name<B: Copy, F, A: JitAlloc> {
            thunk_info: ThunkInfo,
            jit_alloc: A,
            closure: Pin<Box<F>>,
            phantom: PhantomData<B>,
        }

        impl<B: Copy, F, A: JitAlloc> $ty_name<B, F, A> {
            /// Wraps `fun`, producing a bare function with calling convention
            /// `cconv`.
            ///
            /// Uses `jit_alloc` to allocate the W^X memory used to create the thunk.
            #[allow(unused_variables)]
            pub fn with_jit_alloc<CC>(
                cconv: CC,
                fun: F,
                jit_alloc: A,
            ) -> Result<Self, JitAllocError>
            where
                (CC, F): $trait_ident<CC, B>,
            {
                let closure = Box::pin(fun);

                // SAFETY:
                // - thunk_template pointer obtained from the correct source
                // - `closure.downcast_ref()` is a valid pointer to `fun`
                let thunk_info = unsafe {
                    create_thunk(
                        <(CC, F)>::$thunk_template,
                        closure.as_ref().get_ref() as *const F as *const _,
                        &jit_alloc,
                    )?
                };
                Ok(Self {
                    thunk_info,
                    jit_alloc,
                    closure,
                    phantom: PhantomData,
                })
            }

            /// Return a bare function pointer that invokes the underlying closure.
            ///
            /// # Safety
            /// While this method is safe, the returned function pointer is not. In particular, it
            /// must not be called when:
            /// - The lifetime of `self` has expired, or `self` has been dropped.
            #[doc = $safety_doc]
            #[inline]
            pub fn bare(&self) -> B {
                // SAFETY: B is a bare function pointer
                unsafe { std::mem::transmute_copy(&self.thunk_info.thunk) }
            }

            /// Leak the underlying closure, returning the unsafe bare function pointer that invokes it.
            ///
            /// `self` must be `'static` for this method to be called.
            ///
            /// # Safety
            /// While this method is safe, the returned function pointer is not. In particular, it
            /// must not be called when:
            #[doc = $safety_doc]
            #[inline]
            pub fn leak(self) -> B
            where
                Self: 'static,
            {
                ManuallyDrop::new(self).bare()
            }
        }

        impl<B: Copy, F, A: JitAlloc> Drop for $ty_name<B, F, A> {
            fn drop(&mut self) {
                // Don't panic on allocator failures for safety reasons
                // SAFETY:
                // alloc_base is RX memory previously allocated by jit_alloc which has not been
                // freed yet
                unsafe { self.jit_alloc.release(self.thunk_info.alloc_base).ok() };
            }
        }

        #[cfg(feature = "bundled-jit-alloc")]
        impl<B: Copy, F> $ty_name<B, F, jit_alloc::GlobalJitAlloc> {
            /// Wraps `fun`, producing a bare function with calling convention `cconv`.
            ///
            /// The W^X memory required is allocated using the global JIT allocator.
            #[inline]
            pub fn new<CC>(cconv: CC, fun: F) -> Self
            where
                (CC, F): $trait_ident<CC, B>,
            {
                Self::with_jit_alloc(cconv, fun, Default::default()).unwrap()
            }

            cc_shorthand!(new_c, $trait_ident, cc::C, "C");

            cc_shorthand!(new_system, $trait_ident, cc::System, "system");

            #[cfg(all(not(windows), target_arch = "x86_64"))]
            cc_shorthand!(new_sysv64, $trait_ident, cc::Sysv64, "sysv64");

            #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
            cc_shorthand!(new_aapcs, $trait_ident, cc::Aapcs, "aapcs");

            #[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
            cc_shorthand!(new_fastcall, $trait_ident, cc::Fastcall, "fastcall");

            #[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
            cc_shorthand!(new_stdcall, $trait_ident, cc::Stdcall, "stdcall");

            #[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
            cc_shorthand!(new_cdecl, $trait_ident, cc::Cdecl, "cdecl");

            #[cfg(all(windows, target_arch = "x86"))]
            cc_shorthand!(new_thiscall, $trait_ident, cc::Thiscall, "thiscall");

            #[cfg(all(windows, target_arch = "x86_64"))]
            cc_shorthand!(new_win64, $trait_ident, cc::Win64, "win64");
        }
    };
}

bare_closure!(
    BareFnOnce,
    FnOnceThunk,
    THUNK_TEMPLATE_ONCE,
    "[`FnOnce`]",
    "- The function has been called before."
);
bare_closure!(
    BareFnMut,
    FnMutThunk,
    THUNK_TEMPLATE_MUT,
    "[`FnMut`]",
    "- A borrow induced by a previous call is still active."
);
bare_closure!(BareFn, FnThunk, THUNK_TEMPLATE, "[`Fn`]", "");