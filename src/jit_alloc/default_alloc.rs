#![cfg(feature = "default_jit_alloc")]

use core::alloc::Layout;

use crate::jit_alloc::{JitAlloc, JitAllocError, ProtectJitAccess};

#[cfg(unix)]
type JitAllocBase = jit_alloc::JitAlloc<u8, 16>;
#[cfg(windows)]
type JitAllocBase = jit_alloc::JitAlloc<u16, 1>;

#[cfg(not(feature = "std"))]
static GLOBAL_JIT_ALLOC: spin::Mutex<JitAllocBase> = spin::Mutex::new(JitAllocBase::new());
#[cfg(feature = "std")]
static GLOBAL_JIT_ALLOC: std::sync::Mutex<JitAllocBase> =
    std::sync::Mutex::new(JitAllocBase::new());

impl super::GlobalJitAlloc {
    fn use_alloc<T>(&self, action: impl FnOnce(&mut JitAllocBase) -> T) -> T {
        #[cfg(not(feature = "std"))]
        let mut alloc = GLOBAL_JIT_ALLOC.lock();
        #[cfg(feature = "std")]
        let mut alloc = GLOBAL_JIT_ALLOC.lock().unwrap();
        action(&mut alloc)
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "global_jit_alloc")))]
unsafe impl JitAlloc for super::GlobalJitAlloc {
    fn alloc(&self, layout: Layout) -> Result<(*const u8, *mut u8), JitAllocError> {
        self.use_alloc(|a| a.allocate(layout)).map_err(|_| JitAllocError)
    }

    unsafe fn release(&self, rx_ptr: *const u8, layout: Layout) -> Result<(), JitAllocError> {
        self.use_alloc(|a| unsafe { a.deallocate(rx_ptr, layout) })
            .map_err(|_| JitAllocError)
    }

    #[inline(always)]
    unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess) {
        unsafe {
            protect_jit_memory(ptr, size, access);
        }
    }

    #[inline(always)]
    unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
        unsafe {
            flush_instruction_cache(rx_ptr, size);
        }
    }
}

#[cfg(feature = "std")]
pub(super) mod thread_jit_alloc {
    use core::{cell::RefCell, marker::PhantomData};
    use std::thread_local;

    #[allow(unused_imports)]
    use super::*;

    thread_local! {
        static THREAD_JIT_ALLOC: RefCell<JitAllocBase> = const { RefCell::new(JitAllocBase::new()) };
    }

    /// Marker type providing access to a thread-local JIT allocator.
    ///
    /// Unlike [`GlobalJitAlloc`], this allocator is neither [`Send`] nor [`Sync`].
    #[derive(Default, Clone)]
    pub struct ThreadJitAlloc(PhantomData<*mut ()>);

    unsafe impl JitAlloc for ThreadJitAlloc {
        fn alloc(&self, layout: Layout) -> Result<(*const u8, *mut u8), JitAllocError> {
            THREAD_JIT_ALLOC
                .with_borrow_mut(|a| a.allocate(layout))
                .map_err(|_| JitAllocError)
        }

        unsafe fn release(&self, rx_ptr: *const u8, layout: Layout) -> Result<(), JitAllocError> {
            THREAD_JIT_ALLOC
                .with_borrow_mut(|a| unsafe { a.deallocate(rx_ptr, layout) })
                .map_err(|_| JitAllocError)
        }

        #[inline(always)]
        unsafe fn protect_jit_memory(&self, ptr: *const u8, size: usize, access: ProtectJitAccess) {
            unsafe {
                protect_jit_memory(ptr, size, access);
            }
        }

        #[inline(always)]
        unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
            unsafe {
                flush_instruction_cache(rx_ptr, size);
            }
        }
    }
}

unsafe fn protect_jit_memory(_ptr: *const u8, _size: usize, _access: ProtectJitAccess) {
    #[cfg(target_vendor = "apple")]
    unsafe {
        let enabled = matches!(_access, ProtectJitAccess::ReadExecute);
        libc::pthread_jit_write_protect_np(enabled as _);
    }
}

unsafe fn flush_instruction_cache(rx_ptr: *const u8, size: usize) {
    unsafe {
        jit_alloc::clear_cache(rx_ptr, size);
    }
}
