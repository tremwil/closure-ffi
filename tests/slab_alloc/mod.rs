use core::{alloc::Layout, sync::atomic::AtomicUsize};

use closure_ffi::{jit_alloc::ProtectJitAccess, JitAlloc, JitAllocError};
use region::{Allocation, Protection};

#[cfg(feature = "std")]
pub static SLAB: std::sync::LazyLock<SlabAlloc> =
    std::sync::LazyLock::new(|| SlabAlloc::new(0x10000));

#[cfg(not(feature = "std"))]
pub static SLAB: spin::LazyLock<SlabAlloc> = spin::LazyLock::new(|| SlabAlloc::new(0x10000));

#[cfg(feature = "global_jit_alloc")]
closure_ffi::global_jit_alloc!(SLAB);

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

unsafe impl JitAlloc for SlabAlloc {
    fn alloc(&self, layout: Layout) -> Result<(*const u8, *mut u8), JitAllocError> {
        use std::sync::atomic::Ordering::*;

        let offset = self
            .offset
            .try_update(Relaxed, Relaxed, |offset| {
                let start = offset.next_multiple_of(layout.align());
                let end = start + layout.size();
                (end <= self.buf.len()).then_some(end)
            })
            .map_err(|_| JitAllocError)?
            .next_multiple_of(layout.align());

        let ptr = unsafe { self.buf.as_ptr::<u8>().add(offset) };
        Ok((ptr, ptr as *mut _))
    }

    unsafe fn release(&self, _rx_ptr: *const u8, _layout: Layout) -> Result<(), JitAllocError> {
        Ok(())
    }

    unsafe fn flush_instruction_cache(&self, rx_ptr: *const u8, size: usize) {
        clear_cache::clear_cache(rx_ptr, rx_ptr.add(size));
    }

    unsafe fn protect_jit_memory(&self, _ptr: *const u8, _size: usize, _access: ProtectJitAccess) {
        #[cfg(target_os = "macos")]
        unsafe {
            let enabled = matches!(_access, ProtectJitAccess::ReadExecute);
            libc::pthread_jit_write_protect_np(enabled as _);
        }
    }
}
