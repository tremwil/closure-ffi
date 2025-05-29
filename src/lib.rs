#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(feature = "no_std", no_std)]
#![cfg_attr(feature = "unstable", feature(unsize))]
#![doc = include_str!("../README.md")]

#[cfg(all(
    not(target_arch = "x86_64"),
    not(target_arch = "x86"),
    not(target_arch = "aarch64"),
    not(target_arch = "arm")
))]
compile_error!("closure-ffi is not supported on this target architecture.");

#[cfg(all(feature = "bundled_jit_alloc", feature = "custom_jit_alloc"))]
compile_error!("only one of bundled_jit_alloc or custom_jit_alloc may be specified");

#[cfg(feature = "no_std")]
extern crate alloc;

// Provide a no_std agnostic `Box` import for other modules
#[cfg(feature = "no_std")]
pub(crate) use alloc::boxed::Box;
#[cfg(not(feature = "no_std"))]
pub(crate) use std::boxed::Box;

#[doc(hidden)]
pub mod arch;

pub mod bare_closure;
pub mod cc;
pub mod jit_alloc;
pub mod traits;

/// Common imports required to use `closure-ffi`.
pub mod prelude {
    #[doc(inline)]
    pub use super::bare_closure::{
        BareFn, BareFnAny, BareFnMut, BareFnMutAny, BareFnMutSend, BareFnOnce, BareFnOnceAny,
        BareFnOnceSend, BareFnSend,
    };
    #[doc(inline)]
    pub use super::cc;
    #[doc(inline)]
    pub use super::jit_alloc::{JitAlloc, JitAllocError};
}

#[doc(inline)]
pub use prelude::*;
