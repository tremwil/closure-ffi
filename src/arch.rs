//! Architecture-specific code used to implement the code generation making
//! closure-ffi possible.
//!
//! While parts of this module are public for macro reasons, they should not be used directly.

use core::{alloc::Layout, debug_assert_matches};

use crate::jit_alloc::{JitAlloc, JitAllocError, ProtectJitAccess};

#[cfg_attr(target_arch = "aarch64", path = "arch/aarch64.rs")]
#[cfg_attr(target_arch = "arm", path = "arch/arm.rs")]
#[cfg_attr(target_arch = "x86", path = "arch/x86.rs")]
#[cfg_attr(target_arch = "x86_64", path = "arch/x86_64.rs")]
mod arch_impl;

pub use arch_impl::THUNK_ALIGN;

pub struct AllocatedThunk<J: JitAlloc> {
    rx: *const u8,
    thunk_size: usize,
    alloc: J,
}

impl<J: JitAlloc> AllocatedThunk<J> {
    pub fn new_zst_in(alloc: J) -> Self {
        Self {
            rx: core::ptr::without_provenance(THUNK_ALIGN),
            thunk_size: 0,
            alloc,
        }
    }

    pub fn try_new_in<F>(alloc: J, thunk_size: usize, init: F) -> Result<Self, JitAllocError>
    where
        F: FnOnce(*const u8, *mut u8),
    {
        let (rx, rw) = alloc
            .alloc(Layout::from_size_align(thunk_size, THUNK_ALIGN).expect("invalid layout"))?;

        unsafe {
            alloc.protect_jit_memory(rx, thunk_size, ProtectJitAccess::ReadWrite);
        }

        struct DropGuard<'a, J: JitAlloc>(&'a AllocatedThunk<J>);
        impl<J: JitAlloc> Drop for DropGuard<'_, J> {
            fn drop(&mut self) {
                unsafe {
                    self.0.alloc.protect_jit_memory(
                        self.0.rx,
                        self.0.thunk_size,
                        ProtectJitAccess::ReadExecute,
                    );
                }
            }
        }

        let this = AllocatedThunk {
            rx,
            thunk_size,
            alloc,
        };

        let guard = DropGuard(&this);
        init(rx, rw);

        core::mem::forget(guard);
        Ok(this)
    }
}

impl<J: JitAlloc> Drop for AllocatedThunk<J> {
    fn drop(&mut self) {
        if self.thunk_size == 0 {
            // On ARM, clear the thumb mode bit if set
            if cfg!(target_arch = "arm") {
                self.rx = self.rx.map_addr(|a| a & !1);
            }

            unsafe {
                debug_assert_matches!(
                    self.alloc.release(
                        self.rx,
                        Layout::from_size_align_unchecked(self.thunk_size, THUNK_ALIGN),
                    ),
                    Ok(_),
                    "closure-ffi thunk deallocation failed"
                );
            }
        }
    }
}
