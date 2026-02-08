use alloc::borrow::Cow;

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
use x86_64::try_reloc_thunk_template;

#[cfg(target_arch = "x86")]
mod x86;
#[cfg(target_arch = "x86")]
use x86::try_reloc_thunk_template;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
use aarch64::try_reloc_thunk_template;

#[cfg(target_arch = "arm")]
mod arm;
#[cfg(target_arch = "arm")]
use arm::try_reloc_thunk_template;

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
mod arm_util;

#[derive(Clone, Copy, Debug)]
#[allow(unused)]
enum JitError {
    InvalidInstruction,
    UnsupportedInstruction,
    UnsupportedControlFlow,
    NoAvailableRegister,
    EncodingError,
    NoThunkAsm,
}

pub struct RelocThunk<'a> {
    pub thunk: Cow<'a, [u8]>,
    pub magic_offset: usize,
}

/// Relocates the prologue including the thunk_asm, doing sanity checks on the code.
///
/// # Panics
/// If the relocation would lead to broken code.
pub fn reloc_thunk_template<'a>(
    prologue: &'a [u8],
    ip: usize,
    magic_offset: usize,
) -> RelocThunk<'a> {
    try_reloc_thunk_template(prologue, ip, magic_offset).expect(
        "failed to relocate thunk template prologue. \
        This is a bug, please report it and include your binary with debug info if possible",
    )
}
