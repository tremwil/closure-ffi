//! Provides wrappers around closures which allows them to be called through context-free unsafe
//! bare functions.
//!
//! Context-free bare functions are not needed very often, as properly designed C APIs typically
//! allow the user to specify an opaque pointer to a context object which will be provided to the
//! function pointer. However, this is not always the case, and may be impossible in less common
//! scenarios, e.g. function hooking for game modding/hacking. For example:
//!
//! ```
//! use closure_ffi::{cc, BareFnMut};
//!
//! // Imagine we have an foerign C API for reigstering and unregistering some callback function.
//! // Notably, the API does not let the user provide a context object to the callback.
//! unsafe extern "C" ffi_register_callback(cb: unsafe extern "C" fn(u32)) {
//!     // ...
//! }
//! unsafe extern "C" ffi_unregister_callback(cb: unsafe extern "C" fn(u32)) {
//!     // ...
//! }
//!
//! // We want to keep track of sum of callback arguments without using statics. This is where
//! // closure-ffi comes in:
//! let mut sum = 0;
//! let wrapped = BareFnMut::new(cc::C, move |x: u32| {
//!     sum += x;
//! });
//!
//! // Safety: Here, we assert that the foreign API won't use the callback
//! // in ways that break Rust's safety rules. Namely:
//! // - The exclusivity of the FnMut's borrow is respected.
//! // - If the calls are made from a different thread, the closure is Sync.
//! // - We unregister the callback before the BareFnMut is dropped.
//! unsafe {
//!     ffi_register_callback(wrapped.bare());
//!     // Do something that triggers the callback...
//!     ffi_unregister_callback(wrapped.bare());
//! }
//! ```

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
#[doc(hidden)]
pub mod thunk;

pub mod bare_closure;
pub mod cc;
pub mod jit_alloc;

pub mod prelude {
    pub use super::bare_closure::{BareFn, BareFnMut, BareFnOnce};
    pub use super::cc;
    pub use super::jit_alloc::{JitAlloc, JitAllocError};
}

pub use prelude::*;
