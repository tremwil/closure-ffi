#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "unstable", feature(unsize))]
#![cfg_attr(feature = "unstable", feature(ptr_metadata))]
#![cfg_attr(feature = "tuple_trait", feature(tuple_trait))]
#![cfg_attr(feature = "c_variadic", feature(c_variadic))]
#![cfg_attr(feature = "coverage", feature(coverage_attribute))]
#![doc = include_str!("../README.md")]
#![cfg_attr(doc, doc = include_str!("../CHANGELOG.md"))]

#[cfg(not(feature = "std"))]
extern crate alloc;

// Provide a no_std agnostic `Box` import for other modules
#[cfg(not(feature = "std"))]
pub(crate) use alloc::boxed::Box;
#[cfg(feature = "std")]
pub(crate) use std::boxed::Box;

#[doc(hidden)]
pub mod arch;

pub mod bare_closure;
pub mod cc;
pub mod jit_alloc;
pub mod thunk_factory;
pub mod traits;

/// Common imports required to use `closure-ffi`.
pub mod prelude {
    #[doc(inline)]
    pub use super::bare_closure::{
        BareFn, BareFnAny, BareFnMut, BareFnMutAny, BareFnMutSync, BareFnOnce, BareFnOnceAny,
        BareFnOnceSync, BareFnSync, UntypedBareFn, UntypedBareFnMut, UntypedBareFnOnce,
    };
    #[doc(inline)]
    pub use super::cc;
    #[doc(inline)]
    pub use super::jit_alloc::{JitAlloc, JitAllocError};
}

#[doc(inline)]
pub use prelude::*;
