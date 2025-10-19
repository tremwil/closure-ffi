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

/// Trait implemented by unsafe function pointer types of up to 12 arguments.
///
/// Allows introspection of the function's calling convention, arguments, return type, and provides
/// a `call` method for invoking the function.
///
/// # Limitations
/// The trait cannot be automatically implemented for higher-kinded (i.e. `for <'a> fn`) bare
/// functions. For these, use the [`bare_hrtb!`](crate::bare_hrtb) macro to create a transparent
/// wrapper type which implements the trait. Furthermore, the trait cannot be implemented *at all*
/// for higher-kinded bare functions which have more than 3 independent lifetimes.
///
/// # Safety
/// - The trait *must not* be implemented on a type that is not `#[repr(transparent)]` with a
///   function pointer, i.e. has a different size/alignment.
///
/// - When implemented on a non-function pointer type that is `#[repr(transparent)]` to a function
///   pointer, all associated types ([`CC`][`FnPtr::CC`], [`Args`](FnPtr::Args) and
///   [`Ret`](FnPtr::Ret)) must be consistent with the function pointer.
pub unsafe trait FnPtr: Sized + Copy + Send + Sync {
    /// Marker type for the bare function's calling convention.
    ///
    /// This is *required* to be a ZST; in particular it must be safe to create an
    /// instance via [`core::mem::zeroed`] or a reference via [`core::ptr::dangling`].
    type CC: Default + Copy + Send + Sync;

    #[cfg(all(not(doc), feature = "tuple_trait"))]
    /// The arguments of the function, as a tuple.
    ///
    /// This is a GAT with 3 independent lifetimes to support most higher-kinded bare functions.
    type Args<'a, 'b, 'c>: core::marker::Tuple;

    #[cfg(any(doc, not(feature = "tuple_trait")))]
    #[cfg_attr(docsrs, doc(cfg(all())))]
    /// The arguments of the function, as a tuple.
    ///
    /// This is a GAT with 3 independent lifetimes to support most higher-kinded bare functions.
    ///
    /// When the `tuple_trait` crate feature is enabled, this associated type has a
    /// [`core::marker::Tuple`] bound. Note that this also requires the `tuple_trait` nightly
    /// feature.
    type Args<'a, 'b, 'c>;

    /// The return type of the function.
    ///
    /// This is a GAT with 3 independent lifetimes to support most higher-kinded bare functions.
    type Ret<'a, 'b, 'c>;

    /// Calls self.
    ///
    /// # Safety
    /// The same function-specific safety invariants must be upheld as when calling it directly.
    unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b, 'c>;

    /// Creates `Self` from an untyped pointer.
    ///
    /// # Safety
    /// The untyped pointer must point to a valid instance of `Self`.
    unsafe fn from_ptr(ptr: *const ()) -> Self;

    /// Casts `self` to an untyped pointer.
    fn to_ptr(self) -> *const ();

    /// Creates a [`FnOnceThunk`] implementation from a closure taking the same arguments as this
    /// function pointer.
    fn make_once_thunk<F>(fun: F) -> impl FnOnceThunk<Self>
    where
        F: for<'a, 'b, 'c> PackedFnOnce<'a, 'b, 'c, Self>;

    /// Creates a [`FnMutThunk`] implementation from a closure taking the same arguments as this
    /// function pointer.
    fn make_mut_thunk<F>(fun: F) -> impl FnMutThunk<Self>
    where
        F: for<'a, 'b, 'c> PackedFnMut<'a, 'b, 'c, Self>;

    /// Creates a [`FnThunk`] implementation from a closure taking the same arguments as this
    /// function pointer.
    fn make_thunk<F>(fun: F) -> impl FnThunk<Self>
    where
        F: for<'a, 'b, 'c> PackedFn<'a, 'b, 'c, Self>;
}

/// Trait implemented by (`CC`, [`FnOnce`]) tuples used to generate a bare function thunk template,
/// where `CC` is a calling convention marker type.
///
/// # Safety
/// This trait is internal to the library and is not meant to be directly implemented by downstream
/// crates.
///
/// # Why implement on tuples?
/// Implementing on (calling-convention, closure) tuples instead of only the closure is done for two
/// reasons:
/// - It allows type-inferring `B` from the calling convention and a closure with annotated
///   parameters, which is highly desirable for the constructor API of [`BareFn`](crate::BareFn) and
///   friends.
/// - The [`bare_hrtb`](crate::bare_hrtb) macro can be used by downstream crates to generate
///   implementations of this trait for specific higher-ranked bare functions. This is only possible
///   if a local type is present in both the trait generic parameters and the implementor. By
///   implementing on tuples, we can thus use a local type for the calling convention.
pub unsafe trait FnOnceThunk<B: FnPtr>: Sized {
    /// Type-erased bare function thunk template calling `self` by move. Internal to the library.
    const THUNK_TEMPLATE_ONCE: *const u8;

    /// Calls the closure making up this [`FnOnceThunk`] by value.
    ///
    /// # Safety
    /// The same function-specific safety invariants must be upheld as when calling the underlying
    /// closure directly.
    unsafe fn call_once<'a, 'b, 'c>(self, args: B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>;
}

/// Trait implemented by (`CC`, [`FnMut`]) tuples used to generate a bare function thunk template,
/// where `CC` is a calling convention marker type.
///
/// We include `CC` in the type parameters even though it can be fetched from `B` has it enables
/// much richer type inference in the construction API of [`BareFn`](crate::BareFn) and friends.
///
/// # Safety
/// This trait is internal to the library and is not meant to be directly implemented by downstream
/// crates.
///
/// # Why implement on tuples?
/// Implementing on (calling-convention, closure) tuples instead of only the closure is done for two
/// reasons:
/// - It allows type-inferring `B` from the calling convention and a closure with annotated
///   parameters, which is highly desirable for the constructor API of [`BareFn`](crate::BareFn) and
///   friends.
/// - The [`bare_hrtb`](crate::bare_hrtb) macro can be used by downstream crates to generate
///   implementations of this trait for specific higher-ranked bare functions. This is only possible
///   if a local type is present in both the trait generic parameters and the implementor. By
///   implementing on tuples, we can thus use a local type for the calling convention.
pub unsafe trait FnMutThunk<B: FnPtr>: FnOnceThunk<B> {
    /// Type-erased bare function thunk template calling `self` by mutable reference. Internal to
    /// the library.
    const THUNK_TEMPLATE_MUT: *const u8;

    /// Calls the closure making up this [`FnMutThunk`] by mutable reference.    
    ///
    /// # Safety
    /// The same function-specific safety invariants must be upheld as when calling the underlying
    /// closure directly.
    unsafe fn call_mut<'a, 'b, 'c>(&mut self, args: B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>;
}

/// Trait implemented by (`CC`, [`Fn`]) tuples used to generate a bare function thunk template,
/// where `CC` is a calling convention marker type.
///
/// We include `CC` in the type parameters even though it can be fetched from `B` has it enables
/// much richer type inference in the construction API of [`BareFn`](crate::BareFn) and friends.
///
/// # Safety
/// This trait is internal to the library and is not meant to be directly implemented by downstream
/// crates.
///
/// # Why implement on tuples?
/// Implementing on (calling-convention, closure) tuples instead of only the closure is done for two
/// reasons:
/// - It allows type-inferring `B` from the calling convention and a closure with annotated
///   parameters, which is highly desirable for the constructor API of [`BareFn`](crate::BareFn) and
///   friends.
/// - The [`bare_hrtb`](crate::bare_hrtb) macro can be used by downstream crates to generate
///   implementations of this trait for specific higher-ranked bare functions. This is only possible
///   if a local type is present in both the trait generic parameters and the implementor. By
///   implementing on tuples, we can thus use a local type for the calling convention.
pub unsafe trait FnThunk<B: FnPtr>: FnMutThunk<B> {
    /// Type-erased bare function thunk template calling `self` by immutable reference. Internal to
    /// the library.
    const THUNK_TEMPLATE: *const u8;

    /// Calls the closure making up this [`FnThunk`] by value.
    ///
    /// # Safety
    /// The same function-specific safety invariants must be upheld as when calling the underlying
    /// closure directly.
    unsafe fn call<'a, 'b, 'c>(&self, args: B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>;
}

/// Trait alias for [`FnOnce(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>`](FnOnce).
///
/// This is necessary to express the return type of [`FnPtr::make_once_thunk`].
pub trait PackedFnOnce<'a, 'b, 'c, B: FnPtr>:
    FnOnce(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>
{
}

impl<'a, 'b, 'c, B: FnPtr, F> PackedFnOnce<'a, 'b, 'c, B> for F where
    F: FnOnce(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>
{
}

/// Trait alias for [`FnMut(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>`](FnMut).
///
/// This is necessary to express the return type of [`FnPtr::make_mut_thunk`].
pub trait PackedFnMut<'a, 'b, 'c, B: FnPtr>:
    FnMut(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>
{
}

impl<'a, 'b, 'c, B: FnPtr, F> PackedFnMut<'a, 'b, 'c, B> for F where
    F: FnMut(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>
{
}

/// Trait alias for [`Fn(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>`](Fn).
///
/// This is necessary to express the return type of [`FnPtr::make_thunk`].
pub trait PackedFn<'a, 'b, 'c, B: FnPtr>: Fn(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c> {}

impl<'a, 'b, 'c, B: FnPtr, F> PackedFn<'a, 'b, 'c, B> for F where
    F: Fn(B::Args<'a, 'b, 'c>) -> B::Ret<'a, 'b, 'c>
{
}
