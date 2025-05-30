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

#[cfg(target_arch = "x86")]
pub const THUNK_EXTRA_SIZE: usize = 4 + 5 + 2; // mov closure, mov return, jmp
#[cfg(not(target_arch = "x86"))]
pub const THUNK_EXTRA_SIZE: usize = size_of::<usize>() * 2; // closure addr, return addr

// We have to expose the thunk asm macros to allow the hrtb_cc proc macro to generate more complex
// thunk templates

/// Internal. Do not use.
#[cfg(target_arch = "x86_64")]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            "mov {cl_addr}, [rip + 2f]",
            "jmp [rip + 3f]",
            ".align 8",
            "2:",
            ".8byte {cl_magic}",
            "3:",
            ".8byte 0",
            cl_magic = const { $crate::arch::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            options(nostack)
        );
    };
}

/// Internal. Do not use.
#[cfg(target_arch = "x86")]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            ".align 4",
            "nopl 0(%eax)",
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
            ".8byte 0",
            cl_magic = const { $crate::arch::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack)
        );
    };
}

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
            ".4byte 0",
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
            ".4byte 0",
            cl_magic = const { $crate::arch::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack)
        );
    };
}

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
    const PTR_SIZE: usize = size_of::<usize>();

    // When in thumb mode, the thunk pointer will have the lower bit set to 1. Clear it
    #[cfg(thumb_mode)]
    let thunk_template = thunk_template.map_addr(|a| a & !1);

    // Align to pointer size and search for the magic number to be replaced by the
    // closure address
    let mut offset = thunk_template.align_offset(PTR_SIZE);
    while thunk_template.add(offset).cast::<usize>().read() != CLOSURE_ADDR_MAGIC {
        offset += PTR_SIZE;
    }
    let thunk_size = offset + THUNK_EXTRA_SIZE;

    // Skip initial bytes for proper alignment
    let (rx, rw) = jit.alloc(thunk_size + PTR_SIZE - 1)?;
    let align_offset = rw.add(offset).align_offset(PTR_SIZE);
    let (thunk_rx, rw) = (rx.add(align_offset), rw.add(align_offset));

    jit.protect_jit_memory(thunk_rx, thunk_size, ProtectJitAccess::ReadWrite);

    // Copy the prologue + asm block from the compiler-generated thunk
    core::ptr::copy_nonoverlapping(thunk_template, rw, thunk_size);

    // Write the closure pointer
    rw.add(offset).cast::<*const ()>().write(closure_ptr);

    // On X86, we use a PE/ELF relocation for this
    #[cfg(not(target_arch = "x86"))]
    {
        // Write the jump back to the compiler-generated thunk
        let thunk_return = thunk_template.add(thunk_size);
        // When in thumb mode, set the lower bit to one so we don't switch to A32 mode
        #[cfg(thumb_mode)]
        let thunk_return = thunk_return.map_addr(|a| a | 1);
        rw.add(offset + PTR_SIZE).cast::<*const u8>().write(thunk_return);
    }

    jit.protect_jit_memory(thunk_rx, thunk_size, ProtectJitAccess::ReadExecute);
    jit.flush_instruction_cache(thunk_rx, thunk_size);

    // When in thumb mode, set the lower bit to one so we don't switch to A32 mode
    #[cfg(thumb_mode)]
    let thunk_rx = thunk_rx.map_addr(|a| a | 1);

    Ok(ThunkInfo {
        alloc_base: rx,
        thunk: thunk_rx.cast(),
    })
}

/// Runs the provided closure and returns the result. Guaranteed to not inline its code.
///
/// Necessary to prevent the compiler inlining a closure call into the
/// compiler thunk function, which may bring in some PC-relative static constant loads
/// in the prologue on some architectures (namely arm/aarch64).
#[doc(hidden)]
#[inline(never)]
pub fn _never_inline<R>(f: impl FnOnce() -> R) -> R {
    // Empty asm block is not declared as pure, so may have side-effects
    // Necessary to make inline(never) actually work
    unsafe { core::arch::asm!("") }
    f()
}

pub use _thunk_asm;
