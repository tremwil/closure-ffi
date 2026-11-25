//! Traits that `closure-ffi` uses to power its functionality.

use alloc::boxed::Box;

/// Dummy trait implemented by every type.
///
/// As such, its trait object vtable only contains the drop impl for the type. Used to type erase
/// the closure type in [`BareFn`](crate::BareFn) and friends.
pub trait Any {}
impl<T: ?Sized> Any for T {}

/// Trait to construct a [`Box<dyn Trait>`] from a type implementing
/// [`Unsize<dyn Trait>`](core::marker::Unsize).
///
/// Since [`Unsize<T>`](core::marker::Unsize) is gated behind the `unsize` nightly feature,
/// on stable Rust this trait is only implemented for:
/// - [`dyn Any + '_`](Any)
/// - [`dyn Send + '_`](Send)
/// - [`dyn Sync + '_`](Sync)
/// - `dyn Send + Sync + '_`
///
/// Enable the `unstable` feature to support all possible coercions.
///
/// # Safety
/// - `T` must be some trait object type `dyn Trait + Markers + 'a`, where `Markers` are optional
///   marker traits (e.g. [`Send`] and [`Sync`]).
/// - Given the same `T` as above, implementers must also implement `Trait`, `Markers` and outlive
///   `'a`.
pub unsafe trait ToBoxedDyn<T: ?Sized> {
    /// Constructs a [`Box<T>`] from `Self`, coercing into the unsized type.
    fn to_boxed_unsize(value: Self) -> Box<T>;
}

#[cfg(not(feature = "unstable"))]
unsafe impl<'a, T: 'a> ToBoxedDyn<dyn Any + 'a> for T {
    fn to_boxed_unsize(value: Self) -> Box<dyn Any + 'a> {
        Box::new(value)
    }
}

#[cfg(not(feature = "unstable"))]
unsafe impl<'a, T: Send + 'a> ToBoxedDyn<dyn Send + 'a> for T {
    fn to_boxed_unsize(value: Self) -> Box<dyn Send + 'a> {
        Box::new(value)
    }
}

#[cfg(not(feature = "unstable"))]
unsafe impl<'a, T: Sync + 'a> ToBoxedDyn<dyn Sync + 'a> for T {
    fn to_boxed_unsize(value: Self) -> Box<dyn Sync + 'a> {
        Box::new(value)
    }
}

#[cfg(not(feature = "unstable"))]
unsafe impl<'a, T: Send + Sync + 'a> ToBoxedDyn<dyn Send + Sync + 'a> for T {
    fn to_boxed_unsize(value: Self) -> Box<dyn Send + Sync + 'a> {
        Box::new(value)
    }
}

#[cfg(feature = "unstable")]
// SAFETY:
// - we restrict T to be a `dyn Trait`, not just any DST,
// - the `Unsize` impl guarantees implementation of `T` by `U`
unsafe impl<T: ?Sized, U: core::marker::Unsize<T>> ToBoxedDyn<T> for U
where
    T: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<T>>,
{
    fn to_boxed_unsize(value: Self) -> Box<T> {
        Box::<U>::new(value)
    }
}

#[cfg(any(doc, not(feature = "tuple_trait")))]
mod tuple_trait {
    /// Marker trait implemented for argument tuples.
    ///
    /// Without the nightly `tuple_trait` feature, this is actually implemented for *all* T.
    /// With the nightly `tuple_trait` feature, it is an alias of [`core::marker::Tuple`].
    #[cfg_attr(docsrs, doc(cfg(all())))]
    pub trait Tuple: Sized {}

    impl<T> Tuple for T {}
}

#[cfg(all(not(doc), feature = "tuple_trait"))]
pub use core::marker::Tuple;

#[cfg(any(doc, not(feature = "tuple_trait")))]
#[doc(inline)]
pub use tuple_trait::Tuple;

use crate::{
    arch::AllocatedThunk,
    jit_alloc::{self, JitAlloc},
};

/// Trait implemented by unsafe function pointers which can be called with arguments `Args`.
///
/// # Safety
/// - The trait *must not* be implemented on a type that is not `#[repr(transparent)]` with a
///   function pointer, i.e. has a different size/alignment.
///
/// - When implemented on a non-function pointer type that is `#[repr(transparent)]` to a function
///   pointer, all associated types must be consistent with the function pointer.
pub unsafe trait CallWith<Args: Tuple>: FnPtr {
    /// The return type of the function.
    type Ret;

    /// Calls self.
    ///
    /// # Safety
    /// The same function-specific safety invariants must be upheld as when calling it directly.
    unsafe fn call(self, args: Args) -> Self::Ret;
}

pub trait ArgsOf<B: CallWith<Self>>: Tuple {}
impl<Args: Tuple, B: CallWith<Args>> ArgsOf<B> for Args {}

/// Trait implemented by unsafe function pointer types of up to 12 arguments.
///
/// # Limitations
/// The trait cannot be automatically implemented for higher-kinded (i.e. `for <'a> fn`) bare
/// functions. For these, use the [`bare_hrtb!`](crate::bare_hrtb) macro to create a transparent
/// wrapper type which implements the trait.
///
/// # Safety
/// - The trait *must not* be implemented on a type that is not `#[repr(transparent)]` with a
///   function pointer, i.e. has a different size/alignment.
///
/// - When implemented on a non-function pointer type that is `#[repr(transparent)]` to a function
///   pointer, ([`CC`][`BareFn::CC`] must be consistent with the function pointer.
pub unsafe trait FnPtr: Sized + Copy + Send + Sync {
    /// Marker type for the bare function's calling convention.
    ///
    /// This is *required* to be a ZST without invariants; in particular it must be safe to create
    /// an instance via [`core::mem::zeroed`] or a reference via [`core::ptr::dangling`].
    type CC: Default + Copy + Send + Sync;

    /// Creates `Self` from an untyped pointer.
    ///
    /// # Safety
    /// The untyped pointer must point to a valid instance of `Self`.
    unsafe fn from_ptr(ptr: *const ()) -> Self;

    /// Casts `self` to an untyped pointer.
    fn to_ptr(self) -> *const ();
}

pub unsafe trait FnOnceThunk<B: FnPtr>: Sized {
    fn jit_once_thunk<J: JitAlloc>(
        alloc: J,
        closure_ptr: *const (),
    ) -> jit_alloc::Result<AllocatedThunk<J>>;
}

pub unsafe trait FnMutThunk<B: FnPtr>: FnOnceThunk<B> {
    fn jit_mut_thunk<J: JitAlloc>(
        alloc: J,
        closure_ptr: *const (),
    ) -> jit_alloc::Result<AllocatedThunk<J>>;
}

pub unsafe trait FnThunk<B: FnPtr>: FnMutThunk<B> {
    fn jit_thunk<J: JitAlloc>(
        alloc: J,
        closure_ptr: *const (),
    ) -> jit_alloc::Result<AllocatedThunk<J>>;
}
