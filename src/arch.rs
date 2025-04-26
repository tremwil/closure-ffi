//! Architecture-specific code used to implement the code generation making
//! closure-ffi possible.
//!
//! While parts of this module are public for macro reasons, they should not be used directly.

use crate::jit_alloc::{JitAlloc, JitAllocError, ProtectJitAccess};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[doc(hidden)]
pub const CLOSURE_ADDR_MAGIC: usize = 0x0ebe4a8e072bdb2a_u64 as usize;

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[doc(hidden)]
pub const CLOSURE_ADDR_MAGIC: usize = 0x0000DEAD0000DEAD_u64 as usize;

// We have to expose the thunk asm macros to allow the hrtb_cc proc macro to generate more complex
// thunk templates

/// Internal. Do not use.
#[cfg(target_arch = "x86_64")]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            "movabsq ${cl_magic}, {cl_addr}",
            "movabsq $1f, {jmp_addr}",
            "jmp *{jmp_addr}",
            ".byte 0xCC", // To account for jmp above being 2 or 3 bytes
            "1:",
            cl_magic = const { $crate::arch::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack, att_syntax)
        );
    };
}
#[cfg(target_arch = "x86_64")]
const THUNK_ASM_EXTRA_BYTES: usize = 8 + 10 + 3;

/// Internal. Do not use.
#[cfg(target_arch = "x86")]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            "mov ${cl_magic}, {cl_addr}",
            "mov $1f, {jmp_addr}",
            "jmp *{jmp_addr}",
            "1:",
            cl_magic = const { $crate::arch::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack, att_syntax)
        );
    };
}
#[cfg(target_arch = "x86")]
const THUNK_ASM_EXTRA_BYTES: usize = 4 + 5 + 2;

/// Internal. Do not use.
#[cfg(target_arch = "aarch64")]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            "ldr {cl_addr}, 1f",
            "ldr {jmp_addr}, 2f",
            "br {jmp_addr}",
            ".align 3",
            "1:",
            ".8byte {cl_magic}",
            "2:",
            ".8byte 3f",
            "3:",
            cl_magic = const { $crate::arch::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack)
        );
    };
}
#[cfg(target_arch = "aarch64")]
const THUNK_ASM_EXTRA_BYTES: usize = 16;

/// Internal. Do not use.
#[cfg(all(target_arch = "arm", not(thumb_mode)))]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            "ldr {cl_addr}, 1f",
            "ldr {jmp_addr}, 2f",
            "bx {jmp_addr}",
            ".align 2",
            "1:",
            ".4byte {cl_magic}",
            "2:",
            ".4byte 3f",
            "3:",
            cl_magic = const { $crate::arch::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack)
        );
    };
}
/// Internal. Do not use.
#[cfg(all(target_arch = "arm", thumb_mode))]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            "ldr {cl_addr}, 1f",
            "ldr {jmp_addr}, 2f",
            "bx {jmp_addr}",
            ".align 2",
            "1:",
            ".4byte {cl_magic}",
            "2:",
            ".4byte 3f+1",
            "3:",
            cl_magic = const { $crate::arch::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack)
        );
    };
}

#[cfg(target_arch = "arm")]
const THUNK_ASM_EXTRA_BYTES: usize = 8;

#[derive(Debug)]
pub(crate) struct ThunkInfo {
    pub alloc_base: *const u8,
    pub thunk: *const (),
}

/// Creates a thunk to a closure from a thunk template.
///
/// # Safety
/// Given a closure of type `F`, the following must hold:
/// - `thunk_template` is a pointer obtained via the associated const of the
///   `crate::thunk::Fn*Thunk<C, B>` trait implemented on (C, F). Namely, it is a bare function that
///   first invokes [`_thunk_asm`] to obtain the closure pointer, then invokes it.
/// - `closure_ptr` is a valid pointer to an initialized instance of `F`.
pub(crate) unsafe fn create_thunk<J: JitAlloc>(
    thunk_template: *const u8,
    closure_ptr: *const (),
    jit: &J,
) -> Result<ThunkInfo, JitAllocError> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let mut offset = 0;
        while thunk_template.add(offset).cast::<usize>().read_unaligned() != CLOSURE_ADDR_MAGIC {
            offset += 1;
        }

        let thunk_size = offset + THUNK_ASM_EXTRA_BYTES;
        let (rx, rw) = jit.alloc(thunk_size)?;

        J::protect_jit_memory(rx, thunk_size, ProtectJitAccess::ReadWrite);

        core::ptr::copy_nonoverlapping(thunk_template, rw, thunk_size);
        rw.add(offset).cast::<*const ()>().write_unaligned(closure_ptr);

        J::protect_jit_memory(rx, thunk_size, ProtectJitAccess::ReadExecute);
        J::flush_instruction_cache(rx, thunk_size);

        Ok(ThunkInfo {
            alloc_base: rx,
            thunk: rx.cast(),
        })
    }
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    {
        const PTR_SIZE: usize = size_of::<usize>();

        // When in thumb mode, the thunk pointer will have the lower bit set to 1. Clear it
        #[cfg(thumb_mode)]
        let thunk_template = thunk_template.map_addr(|a| a & !1);

        let mut offset = thunk_template.align_offset(PTR_SIZE);
        while thunk_template.add(offset).cast::<usize>().read() != CLOSURE_ADDR_MAGIC {
            offset += PTR_SIZE;
        }

        let thunk_size = offset + THUNK_ASM_EXTRA_BYTES;

        // Skip initial bytes for proper alignment
        let (rx, rw) = jit.alloc(thunk_size + PTR_SIZE - 1)?;
        let align_offset = rw.add(offset).align_offset(PTR_SIZE);
        let (thunk_rx, rw) = (rx.add(align_offset), rw.add(align_offset));

        J::protect_jit_memory(thunk_rx, thunk_size, ProtectJitAccess::ReadWrite);

        core::ptr::copy_nonoverlapping(thunk_template, rw, thunk_size);
        rw.add(offset).cast::<*const ()>().write(closure_ptr);

        J::protect_jit_memory(thunk_rx, thunk_size, ProtectJitAccess::ReadExecute);
        J::flush_instruction_cache(thunk_rx, thunk_size);

        // When in thumb mode, set the lower bit to one so we don't switch to A32 mode
        #[cfg(thumb_mode)]
        let thunk_rx = thunk_rx.map_addr(|a| a | 1);

        Ok(ThunkInfo {
            alloc_base: rx,
            thunk: thunk_rx.cast(),
        })
    }
}

pub use _thunk_asm;
