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
        ///
        /// The W^X memory required is allocated using the global JIT allocator.
        #[inline]
        pub fn $fn_name(fun: F) -> Self
        where
            ($cc_ty, F): $trait_ident<$cc_ty, B>,
        {
            Self::new(<$cc_ty>::default(), fun)
        }
    };
}

macro_rules! bare_closure_impl {
    (
        $ty_name:ident,
        $trait_ident:ident,
        $thunk_template:ident,
        $bare_toggle:meta,
        $bare_receiver:ty,
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
                // - `closure` is a valid pointer to `fun`
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

            #[$bare_toggle]
            /// Return a bare function pointer that invokes the underlying closure.
            ///
            /// # Safety
            /// While this method is safe, the returned function pointer is not. In particular, it
            /// must not be called when:
            /// - The lifetime of `self` has expired, or `self` has been dropped.
            #[doc = $safety_doc]
            #[inline]
            pub fn bare(self: $bare_receiver) -> B {
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
                let no_drop = ManuallyDrop::new(self);
                // SAFETY: B is a bare function pointer
                unsafe { std::mem::transmute_copy(&no_drop.thunk_info.thunk) }
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

        #[cfg(any(test, feature = "bundled_jit_alloc"))]
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

// TODO:
// BareFnOnce still needs work.
// In particular, to avoid leaks we need to have the compiler generated thunk
// call `release` on the allocator after it's done running, then drop the allocator.
// Then, to avoid double frees we need `bare` to be taken by value.
//
// At the moment, we simply force leaking for `BareFnOnce` by omitting `bare()`.

bare_closure_impl!(
    BareFnOnce,
    FnOnceThunk,
    THUNK_TEMPLATE_ONCE,
    cfg(any()),
    Self,
    "[`FnOnce`]",
    "- The function has been called before.\n
- The closure is not `Send`, if calling from a different thread than the current one."
);

bare_closure_impl!(
    BareFnMut,
    FnMutThunk,
    THUNK_TEMPLATE_MUT,
    cfg(all()),
    &mut Self,
    "[`FnMut`]",
    "- A borrow induced by a previous call is still active.\n
- The closure is not `Sync`, if calling from a different thread than the current one."
);
bare_closure_impl!(
    BareFn,
    FnThunk,
    THUNK_TEMPLATE,
    cfg(all()),
    &Self,
    "[`Fn`]",
    "- The closure is not `Sync`, if calling from a different thread than the current one."
);

#[cfg(test)]
mod tests {
    #[test]
    fn test_fn_once() {
        use super::BareFnOnce;

        let value = "test".to_owned();
        let bare_closure = BareFnOnce::new_c(move |n: usize| value + &n.to_string());

        // bare() not available on `BareFnOnce` yet
        let bare = bare_closure.leak();

        let result = unsafe { bare(5) };
        assert_eq!(&result, "test5");
    }

    #[test]
    fn test_fn_mut() {
        use super::BareFnMut;

        let mut value = "0".to_owned();
        let mut bare_closure = BareFnMut::new_c(|n: usize| {
            value += &n.to_string();
            value.clone()
        });

        let bare = bare_closure.bare();

        let result = unsafe { bare(1) };
        assert_eq!(&result, "01");

        let result = unsafe { bare(2) };
        assert_eq!(&result, "012");
    }

    #[test]
    fn test_fn() {
        use super::BareFn;

        let cell = core::cell::RefCell::new("0".to_owned());
        let bare_closure = BareFn::new_c(|n: usize| {
            *cell.borrow_mut() += &n.to_string();
            cell.borrow().clone()
        });

        let bare = bare_closure.bare();

        let result = unsafe { bare(1) };
        assert_eq!(&result, "01");

        let result = unsafe { bare(2) };
        assert_eq!(&result, "012");
    }
}
