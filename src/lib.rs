#![cfg_attr(feature = "build-docs", feature(doc_auto_cfg))]
#![cfg_attr(feature = "build-docs", feature(doc_cfg))]
#![cfg_attr(feature = "no_std", no_std)]
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

#[doc(hidden)]
pub mod arch;
#[doc(hidden)]
pub mod thunk;

pub mod bare_closure;
pub mod cc;
pub mod jit_alloc;

/// Common imports required to use `closure-ffi`.
pub mod prelude {
    #[cfg(feature = "proc_macros")]
    #[doc(inline)]
    pub use super::bare_closure::bare_dyn;
    #[doc(inline)]
    pub use super::bare_closure::{BareFn, BareFnMut, BareFnOnce};
    #[doc(inline)]
    pub use super::cc;
    #[doc(inline)]
    pub use super::jit_alloc::{JitAlloc, JitAllocError};
}

#[doc(inline)]
pub use prelude::*;
