use crate::jit_alloc::{JitAlloc, JitAllocError};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[doc(hidden)]
pub const CLOSURE_ADDR_MAGIC: usize = 0x0ebe4a8e072bdb2a_u64 as usize;

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[doc(hidden)]
pub const CLOSURE_ADDR_MAGIC: usize = 0x0000DEAD0000DEAD_u64 as usize;

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
            ".align 8",
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
#[cfg(target_arch = "arm")]
#[doc(hidden)]
#[macro_export]
macro_rules! _thunk_asm {
    ($closure_ptr:ident) => {
        ::core::arch::asm!(
            "ldr {cl_addr}, 1f",
            "ldr {jmp_addr}, 2f",
            "b {jmp_addr}",
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
#[cfg(target_arch = "arm")]
const THUNK_ASM_EXTRA_BYTES: usize = 8;

pub(crate) struct ThunkInfo {
    pub alloc_base: *const u8,
    pub thunk: *const (),
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) unsafe fn create_thunk(
    thunk_template: *const u8,
    closure_ptr: *mut (),
    jit: &impl JitAlloc,
) -> Result<ThunkInfo, JitAllocError> {
    let mut offset = 0;
    while thunk_template.add(offset).cast::<usize>().read_unaligned() != CLOSURE_ADDR_MAGIC {
        offset += 1;
    }

    let thunk_size = offset + THUNK_ASM_EXTRA_BYTES;
    let (rx, rw) = jit.alloc(thunk_size)?;

    core::ptr::copy_nonoverlapping(thunk_template, rw, thunk_size);
    rw.add(offset).cast::<*mut ()>().write_unaligned(closure_ptr);

    Ok(ThunkInfo {
        alloc_base: rx,
        thunk: rx.cast(),
    })
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
pub(crate) unsafe fn create_thunk(
    thunk_template: *const u8,
    closure_ptr: *mut (),
    jit: &impl JitAlloc,
) -> Result<ThunkInfo, JitAllocError> {
    use jit_allocator::ProtectJitAccess;

    const PTR_SIZE: usize = std::mem::size_of::<usize>();

    let mut offset = thunk_template.align_offset(PTR_SIZE);
    while thunk_template.add(offset).cast::<usize>().read() != CLOSURE_ADDR_MAGIC {
        offset += PTR_SIZE;
    }

    let thunk_size = offset + THUNK_ASM_EXTRA_BYTES;

    // Skip initial bytes for proper alignment
    let (rx, rw) = jit.alloc(thunk_size + PTR_SIZE - 1)?;
    let align_offset = rw.add(offset).align_offset(PTR_SIZE);
    let (thunk_rx, rw) = (rx.add(align_offset), rw.add(align_offset));

    jit_allocator::protect_jit_memory(ProtectJitAccess::ReadWrite);

    core::ptr::copy_nonoverlapping(thunk_template, rw, thunk_size);
    rw.add(offset).cast::<*mut ()>().write(closure_ptr);

    jit_allocator::protect_jit_memory(ProtectJitAccess::ReadExecute);
    jit_allocator::flush_instruction_cache(thunk_rx, thunk_size);

    Ok(ThunkInfo {
        alloc_base: rx,
        thunk: thunk_rx.cast(),
    })
}

pub(crate) use _thunk_asm;
