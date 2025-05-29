//! Traits that `closure-ffi` uses to power its functionality.

use crate::Box;

/// Dummy trait implemented by every type.
///
/// As such, its trait object vtable only contains the drop impl for the type. Used to type erase
/// the closure type in [`BareFn`](crate::BareFn) and friends.
pub trait Any {}
impl<T: ?Sized> Any for T {}

/// Trait to construct a [`Box<T>`] from a type implementing [`Unsize<T>`](core::marker::Unsize).
///
/// Since [`Unsize<T>`](core::marker::Unsize) is gated behind the `unsize` nightly feature,
/// on stable Rust this trait is only implemented for:
/// - [`dyn Any + '_`](Any)
/// - [`dyn Send + '_`](Send)
/// - [`dyn Sync + '_`](Sync)
/// - `dyn Send + Sync + '_`
///
/// Enable the `unstable` feature to support all possible coercions.
pub trait ToBoxedUnsize<T: ?Sized> {
    /// Constructs a [`Box<T>`] from `Self`, coercing into the unsized type.
    fn to_boxed_unsize(value: Self) -> Box<T>;
}

#[cfg(not(feature = "unstable"))]
impl<'a, T: 'a> ToBoxedUnsize<dyn Any + 'a> for T {
    fn to_boxed_unsize(value: Self) -> Box<dyn Any + 'a> {
        Box::new(value)
    }
}

#[cfg(not(feature = "unstable"))]
impl<'a, T: Send + 'a> ToBoxedUnsize<dyn Send + 'a> for T {
    fn to_boxed_unsize(value: Self) -> Box<dyn Send + 'a> {
        Box::new(value)
    }
}

#[cfg(not(feature = "unstable"))]
impl<'a, T: Sync + 'a> ToBoxedUnsize<dyn Sync + 'a> for T {
    fn to_boxed_unsize(value: Self) -> Box<dyn Sync + 'a> {
        Box::new(value)
    }
}

#[cfg(not(feature = "unstable"))]
impl<'a, T: Send + Sync + 'a> ToBoxedUnsize<dyn Send + Sync + 'a> for T {
    fn to_boxed_unsize(value: Self) -> Box<dyn Send + Sync + 'a> {
        Box::new(value)
    }
}

#[cfg(feature = "unstable")]
impl<T: ?Sized, U: core::marker::Unsize<T>> ToBoxedUnsize<T> for U {
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
/// This trait *must not* be implemented on a type that is not `#[repr(transparent]` with a function
/// pointer, i.e. has a different size/alignment.
pub unsafe trait FnPtr: Clone + Copy {
    /// Calling convention of the bare function, as a ZST marker type.
    type CC: Default;

    /// The arguments of the function, as a tuple.
    ///
    /// This is a GAT with 3 independent lifetimes to support most higher-kinded bare functions.
    type Args<'a, 'b, 'c>
    where
        Self: 'a + 'b + 'c;

    /// The return type of the function.
    ///
    /// This is a GAT with 3 independent lifetimes to support most higher-kinded bare functions.
    type Ret<'a, 'b, 'c>
    where
        Self: 'a + 'b + 'c;

    /// Calls self.
    ///
    /// # Safety
    /// The same function-specific safety invariants must be upheld as when calling it directly.
    unsafe fn call<'a, 'b, 'c>(self, args: Self::Args<'a, 'b, 'c>) -> Self::Ret<'a, 'b, 'c>
    where
        Self: 'a + 'b + 'c;
}

/// Trait implemented by (`CC`, [`FnOnce`]) tuples used to generate a bare function thunk template,
/// where `CC` is a calling convention marker type.
///
/// # Safety
/// This trait is internal to the library and is not meant to be directly implemented by downstream
/// crates.
///
/// # Why implement on tuples?
/// Implenting on (calling-convention, closure) tuples instead of only the closure is done for two
/// reasons:
/// - It allows type-infering `B` from the calling convention and a closure with annotated
///   parameters, which is highly desirable for the constructor API of [`BareFn`](crate::BareFn) and
///   friends.
/// - The [`bare_hrtb`](crate::bare_hrtb) macro can be used by downstream crates to generate
///   implementations of this trait for specific higher-ranked bare functions. This is only possible
///   if a local type is present in both the trait generic parameters and the implementor. By
///   implementing on tuples, we can thus use a local type for the calling convention.
pub unsafe trait FnOnceThunk<B: FnPtr> {
    /// Type-erased bare function thunk template calling `self` by move. Internal to the library.
    const THUNK_TEMPLATE_ONCE: *const u8;
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
/// Implenting on (calling-convention, closure) tuples instead of only the closure is done for two
/// reasons:
/// - It allows type-infering `B` from the calling convention and a closure with annotated
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
/// Implenting on (calling-convention, closure) tuples instead of only the closure is done for two
/// reasons:
/// - It allows type-infering `B` from the calling convention and a closure with annotated
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
}
