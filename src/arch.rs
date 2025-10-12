//! Architecture-specific code used to implement the code generation making
//! closure-ffi possible.
//!
//! While parts of this module are public for macro reasons, they should not be used directly.

use crate::jit_alloc::{JitAlloc, JitAllocError, ProtectJitAccess};
#[cfg(feature = "safe_jit")]
use crate::safe_jit::RelocThunk;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[doc(hidden)]
pub mod consts {
    // Define it as 2 u64s to use with inline asm const directive
    pub(super) type Magic = [u64; 2];
    pub const CLOSURE_ADDR_MAGIC: Magic = {
        // lock (x14) push rax/eax, which is invalid no matter the offset into the sequence
        // Credit: https://github.com/Dasaav-dsv/
        #[repr(C)]
        struct LockPushRax([u8; 14], [u8; 2]);
        // SAFETY: bit pattern is valid for dest type and sizes match
        unsafe { core::mem::transmute_copy(&LockPushRax([0xF0; 14], [0xFF, 0xF0])) }
    };

    #[cfg(target_arch = "x86")]
    mod inner {
        pub const THUNK_EXTRA_SIZE: isize = -4;
        pub const CLOSURE_ADDR_OFFSET: isize = -15;
        pub const THUNK_CODE_OFFSET: isize = -16;
    }

    #[cfg(target_arch = "x86_64")]
    mod inner {
        pub const THUNK_RETURN_OFFSET: usize = size_of::<super::Magic>();
        pub const THUNK_EXTRA_SIZE: isize = THUNK_RETURN_OFFSET as isize;
        pub const CLOSURE_ADDR_OFFSET: isize = 0;
    }

    pub(crate) use inner::*;
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[doc(hidden)]
pub mod consts {
    pub(super) type Magic = usize;

    // UDF 0xDEAD repeated twice
    #[cfg(target_arch = "aarch64")]
    pub const CLOSURE_ADDR_MAGIC: Magic = 0x0000DEAD0000DEAD_u64 as usize;

    // UDF 0xDEAD
    #[cfg(all(target_arch = "arm", not(thumb_mode)))]
    pub const CLOSURE_ADDR_MAGIC: Magic = 0xE7FDEAFD;

    // UDF #42 repeated twice
    #[cfg(all(target_arch = "arm", thumb_mode))]
    pub const CLOSURE_ADDR_MAGIC: Magic = 0xDE2ADE2A;

    pub(super) const THUNK_RETURN_OFFSET: usize = 2 * size_of::<usize>();
    pub(super) const THUNK_EXTRA_SIZE: isize = THUNK_RETURN_OFFSET as isize;
    pub(super) const CLOSURE_ADDR_OFFSET: isize = 0;
}

// We perform aligned reads at the pointer size, so make sure the align is sufficient
const _ASSERT_MAGIC_TYPE_SUFFICIENT_ALIGN: () =
    assert!(align_of::<consts::Magic>() >= align_of::<usize>());

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
            "jmp [rip + 2f+$8]",
            ".balign 8, 0xCC",
            "2:",
            ".8byte {cl_magic_0}",
            ".8byte {cl_magic_1}",
            cl_magic_0 = const { $crate::arch::consts::CLOSURE_ADDR_MAGIC[0] },
            cl_magic_1 = const { $crate::arch::consts::CLOSURE_ADDR_MAGIC[1] },
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
            ".balign 8",
            "movl $0xC0DEC0DE, {cl_addr}",
            "movl $1f, {jmp_addr}",
            "jmp *{jmp_addr}",
            ".4byte 0xCCCCCCCC",
            ".8byte {cl_magic_0}",
            ".8byte {cl_magic_1}",
            "1:",
            cl_magic_0 = const { $crate::arch::consts::CLOSURE_ADDR_MAGIC[0] },
            cl_magic_1 = const { $crate::arch::consts::CLOSURE_ADDR_MAGIC[1] },
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
            ".balign 8",
            "1:",
            ".8byte {cl_magic}",
            "2:",
            ".8byte 0",
            cl_magic = const { $crate::arch::consts::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack)
        );
    };
}

/// Internal. Do not use.
#[cfg(target_arch = "arm")]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            "ldr {cl_addr}, 1f",
            "ldr {jmp_addr}, 2f",
            "bx {jmp_addr}",
            ".balign 4",
            "1:",
            ".4byte {cl_magic}",
            "2:",
            ".4byte 0",
            cl_magic = const { $crate::arch::consts::CLOSURE_ADDR_MAGIC },
            cl_addr = out(reg) $closure_ptr,
            jmp_addr = out(reg) _,
            options(nostack)
        );
    };
}

#[derive(Debug)]
pub(crate) struct AllocatedThunk<J: JitAlloc> {
    alloc_base: *const u8,
    thunk: *const (),
    jit: J,
}

impl<J: JitAlloc> Drop for AllocatedThunk<J> {
    fn drop(&mut self) {
        if !self.alloc_base.is_null() {
            let _ = unsafe { self.jit.release(self.alloc_base) };
        }
    }
}

impl<J: JitAlloc> AllocatedThunk<J> {
    /// Gets a pointer to the JITed bare function thunk.
    pub fn thunk_ptr(&self) -> *const () {
        self.thunk
    }

    /// JITs a thunk to a closure from a thunk template.
    ///
    /// Note that if the closure is a ZST, no JIT allocation occurs as the thunk template is a valid
    /// thunk for all instances of the closure.
    ///
    /// # Panics
    /// If the `safe_jit` feature is enabled and the thunk template prologue cannot be relocated.
    ///
    /// # Safety
    /// Given a closure of type `F`, the following must hold:
    /// - `size_of::<F>() == closure_size`.
    /// - `thunk_template` is a pointer obtained via the associated const of the
    ///   `crate::thunk::Fn*Thunk<C, B>` trait implemented on (C, F). Namely, it is:
    ///   - if `F` is not a ZST, a bare function that first invokes [`_thunk_asm`] to obtain the
    ///     closure pointer, then invokes it;
    ///   - if `F` is a ZST, a bare function that creates a dangling closure pointer and invokes it.
    /// - `closure_ptr` is a valid pointer to an initialized instance of `F` (note: may be dandling
    ///   if `F` is a ZST!).
    pub unsafe fn new(
        thunk_template_ptr: *const u8,
        closure_ptr: *const (),
        closure_size: usize,
        jit: J,
    ) -> Result<Self, JitAllocError> {
        if closure_size == 0 {
            return Ok(AllocatedThunk {
                alloc_base: core::ptr::null(),
                thunk: thunk_template_ptr.cast(),
                jit,
            });
        }

        const MAGIC_ALIGN: usize = align_of::<consts::Magic>();

        // When in thumb mode, the thunk pointer will have the lower bit set to 1. Clear it
        #[cfg(thumb_mode)]
        let thunk_template_ptr = thunk_template_ptr.map_addr(|a| a & !1);

        // Align to pointer size and search for the magic number to be replaced by the
        // closure address
        let mut template_magic_offset = thunk_template_ptr.align_offset(MAGIC_ALIGN);
        while thunk_template_ptr.add(template_magic_offset).cast::<consts::Magic>().read()
            != consts::CLOSURE_ADDR_MAGIC
        {
            template_magic_offset += MAGIC_ALIGN;
        }

        let template_size = template_magic_offset.wrapping_add_signed(consts::THUNK_EXTRA_SIZE);
        let thunk_template =
            unsafe { core::slice::from_raw_parts(thunk_template_ptr, template_size) };

        #[cfg(not(feature = "safe_jit"))]
        let (thunk, magic_offset) = (thunk_template, template_magic_offset);

        #[cfg(feature = "safe_jit")]
        let RelocThunk {
            thunk,
            magic_offset,
        } = crate::safe_jit::reloc_thunk_template(
            thunk_template,
            thunk_template_ptr as usize as u64,
            template_magic_offset,
        );

        // Skip initial bytes for proper alignment
        let (rx, rw) = jit.alloc(thunk.len() + MAGIC_ALIGN - 1)?;
        let align_offset = rw.add(magic_offset).align_offset(MAGIC_ALIGN);
        let (thunk_rx, rw) = (rx.add(align_offset), rw.add(align_offset));

        jit.protect_jit_memory(thunk_rx, thunk.len(), ProtectJitAccess::ReadWrite);

        // Copy the prologue + asm block from the compiler-generated thunk
        core::ptr::copy_nonoverlapping(thunk.as_ptr(), rw, thunk.len());

        // Write the closure pointer
        rw.add(magic_offset.wrapping_add_signed(consts::CLOSURE_ADDR_OFFSET))
            .cast::<*const ()>()
            .write_unaligned(closure_ptr);

        // On x86, we use a PE/ELF relocation to load the return address instead
        #[cfg(not(target_arch = "x86"))]
        {
            // Write the jump back to the compiler-generated thunk
            let thunk_return =
                thunk_template_ptr.add(template_magic_offset + consts::THUNK_RETURN_OFFSET);
            // When in thumb mode, set the lower bit to one so we don't switch to A32 mode
            #[cfg(thumb_mode)]
            let thunk_return = thunk_return.map_addr(|a| a | 1);
            rw.add(magic_offset + size_of::<usize>())
                .cast::<*const u8>()
                .write(thunk_return);
        }

        jit.protect_jit_memory(thunk_rx, thunk.len(), ProtectJitAccess::ReadExecute);
        jit.flush_instruction_cache(thunk_rx, thunk.len());

        // When in thumb mode, set the lower bit to one so we don't switch to A32 mode
        #[cfg(thumb_mode)]
        let thunk_rx = thunk_rx.map_addr(|a| a | 1);

        Ok(AllocatedThunk {
            alloc_base: rx,
            thunk: thunk_rx.cast(),
            jit,
        })
    }
}

/// Runs the provided closure and returns the result.
///
/// Depending on the enabled features and architecture, it may be necessary to prevent the compiler
/// from inlining a closure call into the compiler thunk function, which may bring in some
/// PC-relative static constant loads in the prologue on some architectures (namely arm/aarch64).
/// This function controls this behavior.
#[doc(hidden)]
#[cfg(not(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "safe_jit")))]
#[inline(never)]
pub fn _invoke<R>(f: impl FnOnce() -> R) -> R {
    // Empty asm block is not declared as pure, so may have side-effects
    // Necessary to make inline(never) actually work
    unsafe { core::arch::asm!("", options(nostack)) }
    f()
}

#[doc(hidden)]
#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), feature = "safe_jit"))]
#[inline(always)]
pub fn _invoke<R>(f: impl FnOnce() -> R) -> R {
    f()
}

pub use _thunk_asm;
