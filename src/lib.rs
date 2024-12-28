#![cfg_attr(feature = "no_std", no_std)]

#[cfg(all(
    not(target_arch = "x86_64"),
    not(target_arch = "x86"),
    not(target_arch = "aarch64"),
    not(target_arch = "arm")
))]
compile_error!("closure-ffi is not supported on this target architecture.");

#[cfg(feature = "no_std")]
extern crate alloc;

#[doc(hidden)]
pub mod arch;

pub mod bare_closure;
pub mod cc;
pub mod jit_alloc;
pub mod thunk;

pub mod prelude {
    pub use super::bare_closure::{BareFn, BareFnMut, BareFnOnce};
    pub use super::cc;
    pub use super::jit_alloc::{JitAlloc, JitAllocError};
}

pub use prelude::*;
