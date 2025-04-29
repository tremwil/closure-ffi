//! Abstractions around allocators that provide dual-mapped memory with XOR protection rules (one RW
//! view and one RX view) suitable for emitting code at runtime.
//!
//! Meant to be an abstraction over the `jit-allocator` crate's API so that it can be swapped with
//! user-provided allocators.
//!
//! See the [`JitAlloc`] trait for more information.

use core::ops::Deref;

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
pub trait JitAlloc {
    /// Allocates `size` bytes in the executable memory region.
    /// Returns two pointers. One points to Read-Execute mapping and another to Read-Write mapping.
    /// All code writes *must* go to the Read-Write mapping.
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError>;

    /// Releases the memory allocated by `alloc`.
    ///
    /// # Safety
    /// - `rx_ptr` must have been returned from `alloc`
    /// - `rx_ptr` must have been allocated from this allocator
    /// - `rx_ptr` must not have been passed to `release` before
    /// - `rx_ptr` must point to read-execute part of memory returned from `alloc`.
    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError>;

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

impl<J: JitAlloc> JitAlloc for &J {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        (*self).alloc(size)
    }

    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
        (*self).release(rx_ptr)
    }

    #[inline(always)]
    unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
        (*self).flush_instruction_cache(rx_ptr, size);
    }

    #[inline(always)]
    unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess) {
        (*self).protect_jit_memory(ptr, size, access);
    }
}

#[cfg(not(feature = "no_std"))]
impl<J: JitAlloc> JitAlloc for std::sync::LazyLock<J> {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        self.deref().alloc(size)
    }

    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
        self.deref().release(rx_ptr)
    }

    unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
        self.deref().flush_instruction_cache(rx_ptr, size);
    }

    unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess) {
        self.deref().protect_jit_memory(ptr, size, access);
    }
}

#[cfg(feature = "no_std")]
impl<J: JitAlloc, R: spin::RelaxStrategy> JitAlloc for spin::lazy::Lazy<J, fn() -> J, R> {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        self.deref().alloc(size)
    }

    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
        self.deref().release(rx_ptr)
    }

    unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
        self.deref().flush_instruction_cache(rx_ptr, size);
    }

    unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess) {
        self.deref().protect_jit_memory(ptr, size, access);
    }
}

#[cfg(any(feature = "bundled_jit_alloc", feature = "custom_jit_alloc"))]
/// The default, global JIT allocator.
///
/// When the `bundled_jit_alloc` feature is enabled, this is currently implemented as a ZST
/// deffering to a static [`jit_allocator::JitAllocator`] behind a [`std::sync::Mutex`] (or a
/// [`spin::Mutex`] under `no_std`).
///
/// When the `custom_jit_alloc` feature is enabled, defers to a [`JitAlloc`] implementation
/// provided by a downstream crate using the [`global_jit_alloc!`] macro.
///
/// [`spin::Mutex`]: https://docs.rs/spin/0.9/spin/type.Mutex.html
#[derive(Default, Clone, Copy)]
pub struct GlobalJitAlloc;

#[cfg(feature = "bundled_jit_alloc")]
mod bundled_jit_alloc {
    use jit_allocator::JitAllocator;

    use super::*;

    #[inline(always)]
    fn convert_access(access: ProtectJitAccess) -> jit_allocator::ProtectJitAccess {
        match access {
            ProtectJitAccess::ReadExecute => jit_allocator::ProtectJitAccess::ReadExecute,
            ProtectJitAccess::ReadWrite => jit_allocator::ProtectJitAccess::ReadWrite,
        }
    }

    fn flush_instruction_cache(rx_ptr: *const u8, size: usize) {
        #[cfg(all(target_arch = "arm", target_os = "linux"))]
        unsafe {
            const __ARM_NR_CACHEFLUSH: i32 = 0x0f0002;
            libc::syscall(
                __ARM_NR_CACHEFLUSH,
                rx_ptr as usize as u64,
                (rx_ptr as usize + size) as u64,
                0,
            );
            return;
        }
        #[allow(unreachable_code)]
        jit_allocator::flush_instruction_cache(rx_ptr, size);
    }

    impl JitAlloc for core::cell::RefCell<JitAllocator> {
        fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
            self.borrow_mut().alloc(size).map_err(|_| JitAllocError)
        }

        unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
            self.borrow_mut().release(rx_ptr).map_err(|_| JitAllocError)
        }

        #[inline(always)]
        unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
            flush_instruction_cache(rx_ptr, size);
        }

        #[inline(always)]
        unsafe fn protect_jit_memory(
            &self,
            _ptr: *const u8,
            _size: usize,
            access: ProtectJitAccess,
        ) {
            jit_allocator::protect_jit_memory(convert_access(access));
        }
    }

    #[cfg(not(feature = "no_std"))]
    impl JitAlloc for std::sync::RwLock<JitAllocator> {
        fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
            self.write().unwrap().alloc(size).map_err(|_| JitAllocError)
        }

        unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
            self.write().unwrap().release(rx_ptr).map_err(|_| JitAllocError)
        }

        #[inline(always)]
        unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
            flush_instruction_cache(rx_ptr, size);
        }

        #[inline(always)]
        unsafe fn protect_jit_memory(
            &self,
            _ptr: *const u8,
            _size: usize,
            access: ProtectJitAccess,
        ) {
            jit_allocator::protect_jit_memory(convert_access(access));
        }
    }

    #[cfg(not(feature = "no_std"))]
    impl JitAlloc for std::sync::Mutex<JitAllocator> {
        fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
            self.lock().unwrap().alloc(size).map_err(|_| JitAllocError)
        }

        unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
            self.lock().unwrap().release(rx_ptr).map_err(|_| JitAllocError)
        }

        #[inline(always)]
        unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
            flush_instruction_cache(rx_ptr, size);
        }

        #[inline(always)]
        unsafe fn protect_jit_memory(
            &self,
            _ptr: *const u8,
            _size: usize,
            access: ProtectJitAccess,
        ) {
            jit_allocator::protect_jit_memory(convert_access(access));
        }
    }

    #[cfg(feature = "no_std")]
    static GLOBAL_JIT_ALLOC: spin::Mutex<Option<alloc::boxed::Box<JitAllocator>>> =
        spin::Mutex::new(None);
    #[cfg(not(feature = "no_std"))]
    static GLOBAL_JIT_ALLOC: std::sync::Mutex<Option<Box<JitAllocator>>> =
        std::sync::Mutex::new(None);

    impl super::GlobalJitAlloc {
        fn use_alloc<T>(&self, action: impl FnOnce(&mut JitAllocator) -> T) -> T {
            #[cfg(feature = "no_std")]
            let mut maybe_alloc = GLOBAL_JIT_ALLOC.lock();
            #[cfg(not(feature = "no_std"))]
            let mut maybe_alloc = GLOBAL_JIT_ALLOC.lock().unwrap();

            let alloc = maybe_alloc.get_or_insert_with(|| JitAllocator::new(Default::default()));
            action(alloc)
        }
    }

    impl JitAlloc for super::GlobalJitAlloc {
        fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
            self.use_alloc(|a| a.alloc(size)).map_err(|_| JitAllocError)
        }

        unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
            self.use_alloc(|a| a.release(rx_ptr)).map_err(|_| JitAllocError)
        }

        #[inline(always)]
        unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
            flush_instruction_cache(rx_ptr, size);
        }

        #[inline(always)]
        unsafe fn protect_jit_memory(
            &self,
            _ptr: *const u8,
            _size: usize,
            access: ProtectJitAccess,
        ) {
            jit_allocator::protect_jit_memory(convert_access(access));
        }
    }

    #[cfg(not(feature = "no_std"))]
    mod thread_jit_alloc {
        use core::{cell::UnsafeCell, marker::PhantomData};

        use jit_allocator::JitAllocator;

        #[allow(unused_imports)]
        use super::*;

        thread_local! {
            static THREAD_JIT_ALLOC: UnsafeCell<Box<JitAllocator>> =
                UnsafeCell::new(JitAllocator::new(Default::default()));
        }

        /// Marker type providing access to a thread-local JIT allocator.
        ///
        /// Unlike [`GlobalJitAlloc`], this allocator is neither [`Send`] nor [`Sync`].
        #[derive(Default, Clone)]
        pub struct ThreadJitAlloc(PhantomData<*mut ()>);

        impl JitAlloc for ThreadJitAlloc {
            fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
                THREAD_JIT_ALLOC
                    .with(|a| unsafe { &mut *a.get() }.alloc(size))
                    .map_err(|_| JitAllocError)
            }

            unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
                THREAD_JIT_ALLOC
                    .with(|a| unsafe { &mut *a.get() }.release(rx_ptr))
                    .map_err(|_| JitAllocError)
            }

            #[inline(always)]
            unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
                flush_instruction_cache(rx_ptr, size);
            }

            #[inline(always)]
            unsafe fn protect_jit_memory(
                &self,
                _ptr: *const u8,
                _size: usize,
                access: ProtectJitAccess,
            ) {
                jit_allocator::protect_jit_memory(convert_access(access));
            }
        }
    }
    #[cfg(not(feature = "no_std"))]
    pub use thread_jit_alloc::*;
}
#[cfg(feature = "bundled_jit_alloc")]
pub use bundled_jit_alloc::*;

/// Defines a global [`JitAlloc`] implementation which [`GlobalJitAlloc`] will defer to.
///
/// The macro can either take a path to a static variable or an expression resolving to a
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
/// global_jit_alloc!({
///     static WRAPPED_JIT: OnceLock<MyJitAlloc> = OnceLock::new();
///     WRAPPED_JIT.get_or_init(|| MyJitAlloc::new())
/// });
/// ```
#[macro_export]
#[cfg(any(feature = "custom_jit_alloc", feature = "build-docs"))]
#[cfg_attr(feature = "build-docs", doc(cfg(feature = "custom_jit_alloc")))]
macro_rules! global_jit_alloc {
    ($static_var:path) => {
        #[no_mangle]
        extern "Rust" fn _closure_ffi__global_jit_alloc(
        ) -> &'static dyn $crate::jit_alloc::JitAlloc {
            &$static_var
        }
    };
    ($provider:expr) => {
        #[no_mangle]
        extern "Rust" fn _closure_ffi__global_jit_alloc(
        ) -> &'static dyn $crate::jit_alloc::JitAlloc {
            $provider
        }
    };
}
#[cfg(feature = "custom_jit_alloc")]
pub use global_jit_alloc;

#[cfg(feature = "custom_jit_alloc")]
mod custom_jit_alloc {
    use super::{GlobalJitAlloc, JitAlloc, JitAllocError, ProtectJitAccess};

    extern "Rust" {
        fn _closure_ffi__global_jit_alloc() -> &'static dyn JitAlloc;
    }

    fn get_global_jit_alloc() -> &'static dyn JitAlloc {
        unsafe { _closure_ffi__global_jit_alloc() }
    }

    impl JitAlloc for GlobalJitAlloc {
        fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
            get_global_jit_alloc().alloc(size)
        }

        unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
            get_global_jit_alloc().release(rx_ptr)
        }

        unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
            get_global_jit_alloc().flush_instruction_cache(rx_ptr, size);
        }

        unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess) {
            get_global_jit_alloc().protect_jit_memory(ptr, size, access);
        }
    }
}
