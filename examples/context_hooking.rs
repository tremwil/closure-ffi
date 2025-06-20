//! This example shows how one can design the user-facing interface function hooking library with
//! excellent type inference and support for capturing closures using `closure-ffi`.

// This example will not compile without the default JIT allocator.
#![cfg(feature = "default_jit_alloc")]
#![cfg_attr(feature = "tuple_trait", feature(unboxed_closures))]
#![cfg_attr(feature = "tuple_trait", feature(tuple_trait))]
#![cfg_attr(feature = "tuple_trait", feature(fn_traits))]

use core::marker::PhantomData;

use closure_ffi::{
    traits::{FnPtr, FnThunk, ToBoxedUnsize},
    BareFnAny,
};

/// Context object storing information about the current hook.
pub struct HookCtx<'a, B: FnPtr> {
    /// A function pointer to a trampoline that runs the original function's code.
    pub original: B,
    phantom: PhantomData<&'a ()>,
}

// When the `tuple_trait` unstable feature is enabled, we can create a nicer wrapping API by
// creating our own closure type that can be called like the `FnPtr`.
#[cfg(feature = "tuple_trait")]
mod unstable {
    use core::marker::PhantomData;

    use closure_ffi::traits::FnPtr;

    use super::HookCtx;

    struct FnPtrCall<'x, 'y, 'z, B: FnPtr>(B, PhantomData<(&'x mut (), &'y mut (), &'z mut ())>);

    impl<'x, 'y, 'z, B: FnPtr> FnOnce<B::Args<'x, 'y, 'z>> for FnPtrCall<'x, 'y, 'z, B>
    where
        B: 'x + 'y + 'z,
    {
        type Output = B::Ret<'x, 'y, 'z>;

        extern "rust-call" fn call_once(self, args: B::Args<'x, 'y, 'z>) -> Self::Output {
            unsafe { self.0.call(args) }
        }
    }
    impl<'x, 'y, 'z, B: FnPtr> FnMut<B::Args<'x, 'y, 'z>> for FnPtrCall<'x, 'y, 'z, B>
    where
        B: 'x + 'y + 'z,
    {
        extern "rust-call" fn call_mut(&mut self, args: B::Args<'x, 'y, 'z>) -> Self::Output {
            unsafe { self.0.call(args) }
        }
    }
    impl<'x, 'y, 'z, B: FnPtr> Fn<B::Args<'x, 'y, 'z>> for FnPtrCall<'x, 'y, 'z, B>
    where
        B: 'x + 'y + 'z,
    {
        extern "rust-call" fn call(&self, args: B::Args<'x, 'y, 'z>) -> Self::Output {
            unsafe { self.0.call(args) }
        }
    }

    impl<'a, B: FnPtr> HookCtx<'a, B> {
        /// Get an opaque closure type that invokes the original function.
        ///
        /// # Safety
        /// The original function is `unsafe`, yet the value returned by this method allows invoking
        /// it inside a safe context. By calling this, you assert that all invocations of
        /// the return value will satisfy the safety invariants of the wrapped function.
        pub unsafe fn original<'x, 'y, 'z>(
            &self,
        ) -> impl Fn<B::Args<'x, 'y, 'z>, Output = B::Ret<'x, 'y, 'z>>
        where
            Self: 'x + 'y + 'z,
        {
            FnPtrCall(self.original, PhantomData)
        }
    }
}

impl<'a, B: FnPtr> HookCtx<'a, B> {
    /// Calls the original function (stable).
    ///
    /// # Safety
    /// Shares the same safety requirements as the detoured function.
    pub unsafe fn call_original<'x, 'y, 'z>(&self, args: B::Args<'x, 'y, 'z>) -> B::Ret<'x, 'y, 'z>
    where
        Self: 'x + 'y + 'z,
    {
        unsafe { self.original.call(args) }
    }

    /// Get an opaque closure type that invokes the original function.
    ///
    /// # Safety
    /// The original function is `unsafe`, yet the value returned by this method allows invoking
    /// it inside a safe context. By calling this, you assert that all invocations of
    /// the return value will satisfy the safety invariants of the wrapped function.
    #[cfg(not(feature = "tuple_trait"))]
    pub unsafe fn original<'x, 'y, 'z>(&self) -> impl Fn(B::Args<'x, 'y, 'z>) -> B::Ret<'x, 'y, 'z>
    where
        Self: 'x + 'y + 'z,
    {
        let bare = self.original;
        move |args| unsafe { bare.call(args) }
    }
}

/// Skeleton of a generic hooking API for hooking a function of type `B`.
///
/// The main goal is to showcase how we can implement constructors for [`Hook`] that make use of the
/// rich type inference offered by `closure-ffi`.
///
/// Constructors come in two variants:
///
/// - Those that infer the closure type from the bare function type (`B`). These are great when
///   assigning to a field or variable which already has `B` typed. In this example, [`Hook::new`]
///   and [`Hook::with_ctx`] follow this idiom.
///
/// - Those that infer the bare function type (`B`) from a calling convention marker type and the
///   closure's type annotations. These are best used when declaring a short-lived object which
///   doesn't get assigned to an already-typed field. In this example, [`Hook::with_cc`] and
///   [`Hook::with_cc_ctx`] follow this idiom.
pub struct Hook<'a, B: FnPtr> {
    // We use `BareFnAny` to specify stronger bounds (`Send + Sync`) on the type-erased closure.
    // By doing this, `Self` automatically implements `Send` and `Sync`, no need for unsafe impls!
    bare_wrapper: BareFnAny<B, dyn Send + Sync + 'a>,
    // Here we would also store a hanlde to the executable memory we allocated for the trampoline,
    // the target function's address, etc.
}

impl<'a, B: FnPtr> Hook<'a, B> {
    /// Internals not relevant for this example
    fn make_context() -> HookCtx<'a, B> {
        // For the purpose of type inference tests, give out a dangling original `B`
        // (We will not call it)
        HookCtx {
            original: unsafe { B::from_ptr(core::ptr::dangling()) },
            phantom: PhantomData,
        }
    }

    /// Create a hook that will invoke `fun` when the original function is called.
    pub fn new<F>(fun: F) -> Self
    where
        F: ToBoxedUnsize<dyn Send + Sync + 'a>,
        (B::CC, F): FnThunk<B>,
    {
        Self {
            bare_wrapper: BareFnAny::new(fun),
        }
    }

    /// Create a hook to a function of calling convention `cc`, inferring the signature
    /// based on the provided closure.
    pub fn with_cc<CC, F>(cc: CC, fun: F) -> Self
    where
        F: ToBoxedUnsize<dyn Send + Sync + 'a>,
        (CC, F): FnThunk<B>,
    {
        Self {
            bare_wrapper: BareFnAny::with_cc(cc, fun),
        }
    }

    /// Create a hook that will invoke a closure when the original function is called.
    ///
    /// Unlike [`Hook::new`], takes a closure-generating function that is given a hook context
    /// object to capture.
    pub fn with_ctx<F>(ctx_binder: impl FnOnce(HookCtx<'a, B>) -> F) -> Self
    where
        F: ToBoxedUnsize<dyn Send + Sync + 'a>,
        (B::CC, F): FnThunk<B>,
    {
        let ctx = Self::make_context();
        let closure = ctx_binder(ctx);
        Self {
            bare_wrapper: BareFnAny::new(closure),
        }
    }

    /// Create a hook that will invoke a closure when the original function is called.
    ///
    /// The signature of the hooked function is inferred from the calling convention marker type and
    /// the closure's annotations.
    ///
    /// Unlike [`Hook::new`], takes a closure-generating function that is given a hook context
    /// object to capture.
    pub fn with_cc_ctx<CC, F>(cc: CC, ctx_binder: impl FnOnce(HookCtx<'a, B>) -> F) -> Self
    where
        F: ToBoxedUnsize<dyn Send + Sync + 'a>,
        (CC, F): FnThunk<B>,
    {
        let ctx = Self::make_context();
        let closure = ctx_binder(ctx);
        Self {
            bare_wrapper: BareFnAny::with_cc(cc, closure),
        }
    }

    /// Returns the bare function wrapping the hook closure.
    pub fn hook(&self) -> B {
        self.bare_wrapper.bare()
    }
}

/// Showcases the type inference abilities of an API build around `closure-ffi`.
#[test]
fn test_inference() {
    use closure_ffi::cc;

    let borrowed = Box::new(42usize);

    // Infer `F` from `B`

    let hook: Hook<unsafe extern "C" fn(usize) -> usize> = Hook::new(|arg| arg + *borrowed);
    assert_eq!(unsafe { hook.hook()(4) }, 46);

    // Infer `B` from `CC` and `F`
    // hook is Hook<'_, extern "C" fn(usize) -> usize>
    let hook = Hook::with_cc(cc::C, |arg: usize| *borrowed * arg);
    assert_eq!(unsafe { hook.hook()(2) }, 84);

    // Infer 'F` from `B` with unused context

    let hook: Hook<unsafe extern "C" fn(usize) -> u32> = Hook::with_ctx(|_ctx| move |arg| arg as _);
    assert_eq!(unsafe { hook.hook()(42) }, 42);

    // Infer `B` from `CC` and `F` with unused context
    let hook = Hook::with_cc_ctx(cc::C, |_ctx| move |s: String| s.len());
    assert_eq!(unsafe { hook.hook()("abc".to_string()) }, 3);

    // Infer `F` from `B` with used context

    let _hook: Hook<unsafe extern "C" fn(usize, u32) -> u32> = Hook::with_ctx(|ctx| {
        move |x, y| unsafe {
            let result = ctx.call_original((x, y));
            result + 42
        }
    });

    // Infer `B` from `CC` and `F` with used context
    let _hook = Hook::with_cc_ctx(cc::C, |ctx| {
        move |x: usize, y: u32| -> usize {
            // stable API. Must pass the args as a tuple
            #[cfg(not(feature = "tuple_trait"))]
            let result = unsafe { ctx.original()((x, y)) };

            // nightly-only API with tuple_trait feature: call like a normal function
            #[cfg(feature = "tuple_trait")]
            let result = unsafe { ctx.original()(x, y) };

            result + 42
        }
    });
}
