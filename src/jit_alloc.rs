//! Abstractions around allocators that provide dual-mapped memory with XOR protection rules (one RW
//! view and one RX view) suitable for emitting code at runtime.
//!
//! Meant to be an abstraction over the `jit-allocator` crate's API so that it can be swapped with
//! user-provided allocators.
//!
//! See the [`JitAlloc`] trait for more information.

use core::alloc::Layout;
#[allow(unused_imports)]
use core::ops::Deref;

mod default_alloc;
mod jit_tlsf;
mod unix;
mod windows;

/// Anonymous error that may be returned by [`JitAlloc`] implementations when [`JitAlloc::alloc`] or
/// [`JitAlloc::release`] fail.
#[derive(Debug)]
pub struct JitAllocError;

/// Values to use with [`JitAlloc::protect_jit_memory`].
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProtectJitAccess {
    /// Protect JIT memory with Read+Write permissions.
    ReadWrite = 0,
    /// Protect JIT memory with Read+Execute permissions.
    ReadExecute = 1,
}

/// Generic allocator providing virtual memory suitable for emitting code at runtime.
///
/// The API is meant to be a thin abstraction over the `jit-allocator` crate's API, to allow it
/// to be swapped with other allocators.
///
/// # Safety
///
/// The allocator must be sensible and not invalidate memory it gives out prematurely, and must
/// implement all methods correctly.
///
/// See the [`core::alloc::Allocator`] trait for rules of allocation invalidation and specific
/// safety guidelines.
pub unsafe trait JitAlloc {
    /// Allocates bytes in the executable memory region according to the [`Layout`].
    /// Returns two pointers. One points to Read-Execute mapping and another to Read-Write mapping.
    /// All code writes *must* go to the Read-Write mapping.
    ///
    /// The mappings *may overlap* and you should not rely on them being distinct in unsafe code.
    fn alloc(&self, layout: Layout) -> Result<(*const u8, *mut u8), JitAllocError>;

    /// Releases the memory allocated by `alloc`.
    ///
    /// # Safety
    /// - `rx_ptr` must have been returned from `alloc`
    /// - `rx_ptr` must have been allocated from this allocator
    /// - `rx_ptr` must not have been passed to `release` before
    /// - `rx_ptr` must point to read-execute part of memory returned from `alloc`
    /// - `layout` must be the same as was passed to `alloc`
    unsafe fn release(&self, rx_ptr: *const u8, layout: Layout) -> Result<(), JitAllocError>;

    /// On hardened architectures with `MAP_JIT`-like memory flags, set the access for the current
    /// thread.
    ///
    /// This is expected to be a no-op on most platforms, but should be called before writing
    /// or executing JIT memory.
    ///
    /// # Safety
    ///
    /// - `ptr` must point at least `size` bytes of readable memory.
    unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess);

    /// Flushes the instruction cache for (at least) the given slice of executable memory. Should be
    /// called after the JIT memory is ready to be executed.
    ///
    /// On architectures with shared data/instruction caches, like x86_64, this is a no-op.
    ///
    /// # Safety
    /// - `rx_ptr` must point at least `size` bytes of Read-Execute memory.
    unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize);
}

unsafe impl<J: JitAlloc, D: Deref<Target = J>> JitAlloc for D {
    fn alloc(&self, layout: Layout) -> Result<(*const u8, *mut u8), JitAllocError> {
        (**self).alloc(layout)
    }

    unsafe fn release(&self, rx_ptr: *const u8, layout: Layout) -> Result<(), JitAllocError> {
        (**self).release(rx_ptr, layout)
    }

    #[inline(always)]
    unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
        (**self).flush_instruction_cache(rx_ptr, size);
    }

    #[inline(always)]
    unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess) {
        (**self).protect_jit_memory(ptr, size, access);
    }
}

#[cfg(feature = "global_jit_alloc")]
/// The default, global JIT allocator.
///
/// When the `default_jit_alloc` feature is enabled, this is currently implemented as a ZST
/// deferring to a static jit allocator backed by [`rlsf::Tlsf`] behind a [`std::sync::Mutex`]
/// (or a [`spin::Mutex`] under `no_std`).
///
/// When the `default_jit_alloc` feature is not enabled, defers to a [`JitAlloc`] implementation
/// provided by a downstream crate using the [`global_jit_alloc`] macro.
#[derive(Default, Clone, Copy)]
pub struct GlobalJitAlloc;

#[cfg(all(feature = "default_jit_alloc", feature = "std"))]
#[doc(inline)]
pub use default_alloc::thread_jit_alloc::ThreadJitAlloc;

/// Defines a global [`JitAlloc`] implementation which [`GlobalJitAlloc`] will defer to, provided
/// that the `default_jit_alloc` feature is not enabled. Otherwise the latter takes priority.
///
/// The macro can either take a path to a static variable or an unsafe block resolving to a
/// `&'static JitAlloc`:
///
/// ```ignore
/// static GLOBAL_JIT: MyJitAlloc = MyJitAlloc::new();
/// global_jit_alloc!(GLOBAL_JIT);
/// ```
///
/// ```ignore
/// use std::sync::OnceLock;
///
/// global_jit_alloc!(unsafe {
///     static WRAPPED_JIT: OnceLock<MyJitAlloc> = OnceLock::new();
///     WRAPPED_JIT.get_or_init(|| MyJitAlloc::new())
/// });
/// ```
///
/// The block form must be marked with `unsafe` as sometimes returning a different impl can lead to
/// UB, and you are responsible to make sure this doesn't happen.
#[macro_export]
#[cfg(any(docsrs, feature = "global_jit_alloc"))]
#[cfg_attr(docsrs, doc(cfg(feature = "global_jit_alloc")))]
macro_rules! global_jit_alloc {
    (unsafe $provider:block) => {
        #[no_mangle]
        extern "Rust" fn _closure_ffi_3_global_jit_alloc(
        ) -> &'static (dyn $crate::jit_alloc::JitAlloc + Sync) {
            unsafe { $provider }
        }
    };
    ($static_var:path) => {
        #[no_mangle]
        extern "Rust" fn _closure_ffi_3_global_jit_alloc(
        ) -> &'static (dyn $crate::jit_alloc::JitAlloc + Sync) {
            &$static_var
        }
    };
}
#[cfg(any(docsrs, feature = "global_jit_alloc"))]
#[cfg_attr(docsrs, doc(cfg(feature = "global_jit_alloc")))]
pub use global_jit_alloc;

#[cfg(all(feature = "global_jit_alloc", not(feature = "default_jit_alloc")))]
mod custom_jit_alloc {
    use core::alloc::Layout;

    use super::{GlobalJitAlloc, JitAlloc, JitAllocError, ProtectJitAccess};

    extern "Rust" {
        fn _closure_ffi_3_global_jit_alloc() -> &'static (dyn JitAlloc + Sync);
    }

    fn get_global_jit_alloc() -> &'static dyn JitAlloc {
        unsafe { _closure_ffi_3_global_jit_alloc() }
    }

    unsafe impl JitAlloc for GlobalJitAlloc {
        fn alloc(&self, layout: Layout) -> Result<(*const u8, *mut u8), JitAllocError> {
            get_global_jit_alloc().alloc(layout)
        }

        unsafe fn release(&self, rx_ptr: *const u8, layout: Layout) -> Result<(), JitAllocError> {
            get_global_jit_alloc().release(rx_ptr, layout)
        }

        unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
            get_global_jit_alloc().flush_instruction_cache(rx_ptr, size);
        }

        unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess) {
            get_global_jit_alloc().protect_jit_memory(ptr, size, access);
        }
    }
}
