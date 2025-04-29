use core::sync::atomic::AtomicUsize;

use closure_ffi::{JitAlloc, JitAllocError};
use region::{Allocation, Protection};

pub struct SlabAlloc {
    buf: Allocation,
    offset: AtomicUsize,
}

// Safety: SlabAlloc can be moved to another thread.
unsafe impl Send for SlabAlloc {}
// Safety: SlabAlloc references can be passed to other threads.
unsafe impl Sync for SlabAlloc {}

impl SlabAlloc {
    pub fn new(size: usize) -> Self {
        Self {
            buf: region::alloc(size, Protection::all()).unwrap(),
            offset: AtomicUsize::new(0),
        }
    }
}

impl JitAlloc for SlabAlloc {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        use std::sync::atomic::Ordering::*;

        let offset = self
            .offset
            .fetch_update(Relaxed, Relaxed, |offset| {
                (size + offset <= self.buf.len()).then_some(size + offset)
            })
            .map_err(|_| JitAllocError)?;

        let ptr = unsafe { self.buf.as_ptr::<u8>().add(offset) };
        Ok((ptr, ptr as *mut _))
    }

    unsafe fn release(&self, _rx_ptr: *const u8) -> Result<(), JitAllocError> {
        Ok(())
    }

    unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
        #[cfg(not(target_arch = "arm"))]
        clear_cache::clear_cache(rx_ptr, rx_ptr.add(size));
        #[cfg(all(target_arch = "arm", target_os = "linux"))]
        unsafe {
            const __ARM_NR_CACHEFLUSH: i32 = 0x0f0002;
            libc::syscall(
                __ARM_NR_CACHEFLUSH,
                rx_ptr as usize as u64,
                (rx_ptr as usize + size) as u64,
                0,
            );
        }
    }

    unsafe fn protect_jit_memory(
        &self,
        _ptr: *const u8,
        _size: usize,
        _access: closure_ffi::jit_alloc::ProtectJitAccess,
    ) {
        // TODO (MacOS): implement this!
        // Hardened runtime isn't used in macos-latest GitHub actions runners, so leaving this blank
        // is fine for CI testing
    }
}
