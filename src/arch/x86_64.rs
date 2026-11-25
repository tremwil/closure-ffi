use core::{
    arch::{asm, naked_asm},
    mem::MaybeUninit,
};

use crate::{
    arch::AllocatedThunk,
    jit_alloc::{self, JitAlloc},
    macros::variadic_generic,
};

pub const THUNK_ALIGN: usize = 8;

macro_rules! seh {
    ($directive:literal) => {
        cfg_select! {
            windows => $directive,
            _ => "",
        }
    };
}

macro_rules! cfi {
    ($directive:literal) => {
        cfg_select! {
            not(windows) => $directive,
            _ => "",
        }
    };
}

/// x64 general purpose registers.
///
/// Their integer value is equal to their encoding, with the highest bit being REX.R.
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum Gpr {
    Rax = 0,
    Rcx = 1,
    Rdx = 2,
    Rbx = 3,
    Rsp = 4,
    Rbp = 5,
    Rsi = 6,
    Rdi = 7,
    R8 = 8,
    R9 = 9,
    R10 = 10,
    R11 = 11,
    R12 = 12,
    R13 = 13,
    R14 = 14,
    R15 = 15,
}

#[repr(C)]
pub struct RegThunk {
    // mov rN, [rip+closure]
    mov_closure: [u8; 7],
    // jmp qword ptr [rip+thunk]
    jmp_thunk: [u8; 6],
    // .balign 8
    pad: [u8; 3],
    // .8byte closure
    closure: *const (),
    // .8byte thunk
    thunk: *const (),
}

impl RegThunk {
    fn new(closure_reg: Gpr, closure: *const (), thunk: *const ()) -> Self {
        Self {
            mov_closure: {
                let mut ins = *b"\x48\x8B\x05\x09\x00\x00\x00";
                ins[2] |= (closure_reg as u8 & 7) << 3; // modrm.reg
                ins[0] |= (closure_reg as u8 & 8) >> 1; // REX.R
                ins
            },
            jmp_thunk: *b"\xFF\x25\x0B\x00\x00\x00",
            pad: *b"\xCC\xCC\xCC",
            closure,
            thunk,
        }
    }
}

pub mod sysv64 {
    use super::*;

    /// sysv64 gpr argument registers.
    const GPR_ARGS: &[Gpr] = &[Gpr::Rdi, Gpr::Rsi, Gpr::Rdx, Gpr::Rcx, Gpr::R8, Gpr::R9];

    #[repr(C)]
    #[repr(align(32))]
    struct Context<A> {
        _align: [A; 0],
        ptr: *const (),
        // Original return address part of the padding bytes as the very end
        // of the struct.
        //
        // Note that we're playing with fire by having the return address in a stack argument slot,
        // since this memory is owned by the callee in sysv. However, in practice the argument is
        // never moved outside of its stack slot, and any mutations are done in-place. It is
        // also extremely unlikely this will change, as it would always be less efficient.
    }

    macro_rules! thunk_impl {
        (
            $num_args: tt,
            ($($id_tys:ident,)*),
            ($($tuple_idx:tt,)*),
            ($($args:ident: $tys:ty,)*),
            $cconv: ty,
            $cconv_lit: literal
        ) => {
            const _: () = {
                #[repr(C)]
                struct CTuple<$($id_tys,)*>($($tys,)*);

                fn next_avail_gpr<R $(,$id_tys)*>() -> Option<usize> {
                    extern $cconv_lit fn setter<R $(,$id_tys)*>(
                        $(_: MaybeUninit<$tys>,)*
                        arg: usize,
                        out: &mut usize,
                    ) -> MaybeUninit<R> {
                        *out = arg;
                        MaybeUninit::uninit()
                    }

                    #[unsafe(naked)]
                    extern $cconv_lit fn probe<R $(,$id_tys)*>(
                        $(_: MaybeUninit<$tys>,)*
                        gpr: usize,
                        out: &mut usize,
                    ) -> MaybeUninit<R> {
                        naked_asm!(
                            "test rdi, rdi",
                            "jne 2f",
                            "mov rdi, 1",
                            "2:",
                            "test rsi, rsi",
                            "jne 2f",
                            "mov rsi, 2",
                            "2:",
                            "test rdx, rdx",
                            "jne 2f",
                            "mov rdx, 3",
                            "2:",
                            "test rcx, rcx",
                            "jne 2f",
                            "mov rcx, 4",
                            "2:",
                            "test r8, r8",
                            "jne 2f",
                            "mov r9, 5",
                            "2:",
                            "test r9, r9",
                            "jne 2f",
                            "mov r9, 6",
                            "2:",
                            "jmp [rip + {setter}]",
                            setter = sym setter::<R $(,$tys)*>
                        )
                    }

                    let mut result = 0;
                    probe::<R $(,$tys)*>(
                        $(MaybeUninit::<$tys>::uninit(),)*
                        0,
                        &mut result
                    );

                    (result > 0).then(|| result - 1)
                }

                unsafe impl<R, F $(,$id_tys)*> $crate::traits::FnOnceThunk<
                    unsafe extern $cconv_lit fn($($tys,)*) -> R
                > for ($cconv, F) where
                    F: FnOnce($($tys,)*) -> R
                {
                    fn jit_once_thunk<J: JitAlloc>(
                        alloc: J,
                        closure_ptr: *const (),
                    ) -> jit_alloc::Result<AllocatedThunk<J>> {
                        unsafe extern $cconv_lit fn by_reg_thunk<R, F $(,$id_tys)*>(
                            $($args: $tys,)*
                            closure_ptr: *mut F
                        ) -> R where
                            F: FnOnce($($tys,)*) -> R
                        {
                            unsafe {
                                (closure_ptr.read())($($args,)*)
                            }
                        }

                        // for zero-arguments CTuple is not FFI-safe, but it'll also never
                        // be called, so we can safely ignore the warning.
                        #[allow(improper_ctypes_definitions)]
                        unsafe extern $cconv_lit fn by_stack<R, F $(,$id_tys)*>(
                            ctx: Context<CTuple<$($tys,)*>>
                            $(,$args: $tys)*
                        ) -> R where
                            F: FnOnce($($tys,)*) -> R
                        {
                            unsafe {
                                (ctx.ptr.cast::<F>().read())($($args,)*)
                            }
                        }

                        #[unsafe(naked)]
                        unsafe extern $cconv_lit fn by_stack_thunk<R, F $(,$id_tys)*>(
                            $($args: $tys,)*
                        ) -> R where
                            F: FnOnce($($tys,)*) -> R
                        {
                            naked_asm!(
                                cfi!(".cfi_startproc {self_name}"),
                                seh!(".seh_proc {self_name}"),
                                "sub rsp, {stack_adjust}",
                                cfi!(".cfi_adjust_cfa_offset {stack_adjust}"),
                                seh!(".seh_stackalloc {stack_adjust}; .seh_endprologue"),
                                "mov [rsp], rax",
                                "call {dispatcher}",
                                "add rsp, {stack_adjust}",
                                "ret",
                                cfi!(".cfi_endproc"),
                                seh!(".seh_endproc"),
                                stack_adjust = const {
                                    size_of::<Context<CTuple<$($tys,)*>>>() - 8
                                },
                                self_name = sym by_stack_thunk::<R, F $(,$tys)*>,
                                dispatcher = sym by_stack::<R, F $(,$tys)*>
                            )
                        }

                        let thunk = match next_avail_gpr::<R $(,$tys)*>() {
                            Some(i) => RegThunk::new(
                                GPR_ARGS[i], closure_ptr, by_reg_thunk::<R, F $(,$tys)*> as *const ()
                            ),
                            None => RegThunk::new(
                                Gpr::Rax, closure_ptr, by_stack_thunk::<R, F $(,$tys)*> as *const ()
                            )
                        };

                        AllocatedThunk::try_new_in(
                            alloc,
                            size_of::<RegThunk>(),
                            |_, rw| unsafe { rw.cast::<RegThunk>().write(thunk) }
                        )
                    }
                }
            };
        };
    }

    variadic_generic!(thunk_impl!(crate::cc::Sysv64, "sysv64"));
    variadic_generic!(thunk_impl!(crate::cc::Sysv64Unwind, "sysv64-unwind"));
}

// note: efiapi is effectively the same calling convention.
pub mod win64 {
    use super::*;

    /// win64 gpr argument registers.
    const GPR_ARGS: &[Gpr] = &[Gpr::Rcx, Gpr::Rdx, Gpr::R8, Gpr::R9];

    /// Check whether T will be returned by memory (via a pointer in RCX) or
    /// by register (RAX or a vector register for float types).
    ///
    /// This works by defining a function returning a [`MaybeUninit<T>`] with the first
    /// byte set to 1, and calling it with a via inline assembler with a pointer in RCX.
    /// If it returns by register, the memory pointed to by RCX will be left untouched.
    /// Otherwise, a 1 will be written at the first byte.
    fn is_memory_ret<T>() -> bool {
        // ZSTs never return by memory.
        if size_of::<T>() == 0 {
            return false;
        }

        extern "win64" fn probe<T>() -> MaybeUninit<T> {
            let mut memory = MaybeUninit::<T>::uninit();

            if size_of::<T>() > 0 {
                // SAFETY: T has non-zero size
                unsafe { memory.as_mut_ptr().cast::<u8>().write(1) };
            }
            memory
        }

        let mut return_slot = MaybeUninit::<T>::uninit();
        unsafe {
            // SAFETY: T has non-zero size
            return_slot.as_mut_ptr().cast::<u8>().write(0);
            // SAFETY: If T is returned by memory, `probe::<T>` is provided a pointer
            // to the return value in RCX according to the win64 calling convention.
            //
            // Otherwise, it does not take any parameters.
            asm!(
                "call {}",
                sym probe::<T>,
                in("rcx") return_slot.as_mut_ptr(),
                clobber_abi("win64")
            );
            // SAFETY: T has non-zero size
            return_slot.as_mut_ptr().cast::<u8>().read() != 0
        }
    }

    // 0-2 arguments, closure always passed in register
    macro_rules! by_register {
        (
            $num_args: tt,
            ($($id_tys:ident,)*),
            $_tuple_ids:tt,
            ($($args:ident: $tys:ty,)*),
            $cconv: ty,
            $cconv_lit: literal
        ) => {
            unsafe impl<R, F $(,$id_tys)*> $crate::traits::FnOnceThunk<
                unsafe extern $cconv_lit fn($($tys,)*) -> R
            > for ($cconv, F) where
                F: FnOnce($($tys,)*) -> R
            {
                fn jit_once_thunk<J: JitAlloc>(
                    alloc: J,
                    closure_ptr: *const (),
                ) -> jit_alloc::Result<AllocatedThunk<J>> {
                    unsafe extern $cconv_lit fn thunk<R, F $(,$id_tys)*>(
                        $($args: $tys,)*
                        closure_ptr: *mut F
                    ) -> R where
                        F: FnOnce($($tys,)*) -> R
                    {
                        unsafe {
                            (closure_ptr.read())($($args,)*)
                        }
                    }

                    let register = GPR_ARGS[$num_args + is_memory_ret::<R>() as usize];
                    let thunk = RegThunk::new(register, closure_ptr, thunk::<R, F $(,$tys)*> as *const ());

                    AllocatedThunk::try_new_in(
                        alloc,
                        size_of::<RegThunk>(),
                        |_, rw| unsafe { rw.cast::<RegThunk>().write(thunk) }
                    )
                }
            }
        };
    }

    // special case for 3 arguments
    macro_rules! maybe_by_stack {
        (
            $_id_tys:tt,
            $_tuple_ids:tt,
            $_args:tt,
            $cconv: ty,
            $cconv_lit: literal
        ) => {
            unsafe impl<R, F, T0, T1, T2> $crate::traits::FnOnceThunk<
                unsafe extern $cconv_lit fn(T0, T1, T2) -> R
            > for ($cconv, F) where
                F: FnOnce(T0, T1, T2) -> R
            {
                fn jit_once_thunk<J: JitAlloc>(
                    alloc: J,
                    closure_ptr: *const (),
                ) -> jit_alloc::Result<AllocatedThunk<J>> {
                    unsafe extern $cconv_lit fn thunk<R, F, T0, T1, T2>(
                        a0: T0,
                        a1: T1,
                        a2: T2,
                        closure_ptr: *mut F,
                    ) -> R where
                        F: FnOnce(T0, T1, T2) -> R
                    {
                        unsafe {
                            (closure_ptr.read())(a0, a1, a2)
                        }
                    }

                    #[unsafe(naked)]
                    unsafe extern $cconv_lit fn thunk_by_mem<R, F, T0, T1, T2>(
                        _: T0,
                        _: T1,
                        _: T2
                    ) -> R
                    where
                        F: FnOnce(T0, T1, T2) -> R,
                    {
                        core::arch::naked_asm!(
                            cfi!(".cfi_startproc {self_name}"),
                            seh!(".seh_proc {self_name}"),
                            "sub rsp, 0x28",
                            cfi!(".cfi_adjust_cfa_offset 0x28"),
                            seh!(".seh_stackalloc 0x28"),
                            seh!(".seh_endprologue"),
                            "mov [rsp+0x20], rax",
                            "call {dispatcher}",
                            "add rsp, 0x28",
                            "ret",
                            cfi!(".cfi_endproc"),
                            seh!(".seh_endproc"),
                            self_name = sym thunk_by_mem::<R, F, T0, T1, T2>,
                            dispatcher = sym thunk::<R, F, T0, T1, T2>,
                        )
                    }

                    let (gpr, thunk_ptr) = match is_memory_ret::<R>() {
                        false => (Gpr::R9, thunk::<R, F, T0, T1, T2> as *const ()),
                        true => (Gpr::Rax, thunk_by_mem::<R, F, T0, T1, T2> as *const ())
                    };

                    let thunk = RegThunk::new(gpr, closure_ptr, thunk_ptr);
                    AllocatedThunk::try_new_in(
                        alloc,
                        size_of::<RegThunk>(),
                        |_, rw| unsafe { rw.cast::<RegThunk>().write(thunk) }
                    )
                }
            }
        };
    }

    // 4+ arguments, closure always passed on stack
    macro_rules! by_stack {
        (
            $num_args: tt,
            ($($id_tys:ident,)*),
            $_tuple_ids:tt,
            // tail arguments
            ($($tail_args:ident: $tail_tys:ty,)*),
            $cconv: ty,
            $cconv_lit: literal
        ) => {
            unsafe impl<R, F $(,$id_tys)*> $crate::traits::FnOnceThunk<
                unsafe extern $cconv_lit fn($($id_tys,)*) -> R
            > for ($cconv, F) where
                F: FnOnce($($id_tys,)*) -> R
            {
                fn jit_once_thunk<J: JitAlloc>(
                    alloc: J,
                    closure_ptr: *const (),
                ) -> jit_alloc::Result<AllocatedThunk<J>> {
                    unsafe extern $cconv_lit fn return_by_reg<R, F $(,$id_tys)*>(
                        a0: T0,
                        a1: T1,
                        a2: T2,
                        a3: T3,
                        closure_ptr: *mut F,
                        _ret: usize,
                        _s0: usize,
                        _s1: usize,
                        _s2: usize,
                        _s3: usize,
                        $($tail_args: $tail_tys,)*
                    ) -> R where
                        F: FnOnce($($id_tys,)*) -> R
                    {
                        unsafe {
                            (closure_ptr.read())(a0, a1, a2, a3, $($tail_args,)*)
                        }
                    }

                    #[unsafe(naked)]
                    unsafe extern $cconv_lit fn thunk_by_reg<R, F $(,$id_tys)*>(
                        $(_: $id_tys,)*
                    ) -> R
                    where
                        F: FnOnce($($id_tys,)*) -> R,
                    {
                        core::arch::naked_asm!(
                            cfi!(".cfi_startproc {self_name}"),
                            seh!(".seh_proc {self_name}"),
                            "sub rsp, 0x28",
                            cfi!(".cfi_adjust_cfa_offset 0x28"),
                            seh!(".seh_stackalloc 0x28; .seh_endprologue"),
                            "mov [rsp+0x20], rax",
                            "call {dispatcher}",
                            "add rsp, 0x28",
                            "ret",
                            cfi!(".cfi_endproc"),
                            seh!(".seh_endproc"),
                            self_name = sym thunk_by_reg::<R, F $(,$id_tys)*>,
                            dispatcher = sym return_by_reg::<R, F $(,$id_tys)*>,
                        )
                    }

                    unsafe extern $cconv_lit fn return_by_mem<R, F $(,$id_tys)*>(
                        a0: T0,
                        a1: T1,
                        a2: T2,
                        closure_ptr: *mut F,
                        _ret: usize,
                        _s0: usize,
                        _s1: usize,
                        _s2: usize,
                        _s3: usize,
                        a3: T3,
                        $($tail_args: $tail_tys,)*
                    ) -> R where
                        F: FnOnce($($id_tys,)*) -> R
                    {
                        unsafe {
                            (closure_ptr.read())(a0, a1, a2, a3, $($tail_args,)*)
                        }
                    }

                    #[unsafe(naked)]
                    unsafe extern $cconv_lit fn thunk_by_mem<R, F $(,$id_tys)*>(
                        $(_: $id_tys,)*
                    ) -> R
                    where
                        F: FnOnce($($id_tys,)*) -> R,
                    {
                        core::arch::naked_asm!(
                            cfi!(".cfi_startproc {self_name}"),
                            seh!(".seh_proc {self_name}"),
                            "sub rsp, 0x28",
                            cfi!(".cfi_adjust_cfa_offset 0x28"),
                            seh!(".seh_stackalloc 0x28"),
                            seh!(".seh_endprologue"),
                            "mov [rsp+0x20], rax",
                            "call {dispatcher}",
                            "add rsp, 0x28",
                            "ret",
                            cfi!(".cfi_endproc"),
                            seh!(".seh_endproc"),
                            self_name = sym thunk_by_mem::<R, F $(,$id_tys)*>,
                            dispatcher = sym return_by_mem::<R, F $(,$id_tys)*>,
                        )
                    }

                    let stack_adjust_thunk = match is_memory_ret::<R>() {
                        false => thunk_by_reg::<R, F $(,$id_tys)*> as *const (),
                        true => thunk_by_mem::<R, F $(,$id_tys)*> as *const ()
                    };

                    let thunk = RegThunk::new(Gpr::Rax, closure_ptr, stack_adjust_thunk);
                    AllocatedThunk::try_new_in(
                        alloc,
                        size_of::<RegThunk>(),
                        |_, rw| unsafe { rw.cast::<RegThunk>().write(thunk) }
                    )
                }
            }
        };
    }

    macro_rules! thunk_impl {
        // for 0-2 arguments, we'll always have a register available, so we can skip
        // generating the two different versions of the thunk depending on whether
        // the return value is passed by memory or by stack.
        (0, $($tokens:tt)*) => {
            by_register!(0, $($tokens)*);
        };
        (1, $($tokens:tt)*) => {
            by_register!(1, $($tokens)*);
        };
        (2, $($tokens:tt)*) => {
            by_register!(2, $($tokens)*);
        };
        // for 3 arguments, the extra argument _may_ have to go on the stack.
        (3, $($tokens:tt)*) => {
            maybe_by_stack!($($tokens)*);
        };
        // for 4+ arguments, the extra argument going on the stack is guaranteed.
        (
            $num_args: tt,
            $id_tys:tt,
            $tuple_ids:tt,
            (
                a0:$t0:ty, a1:$t1:ty, a2:$t2:ty, a3:$t3:ty,
                $($tail_args:ident: $tail_tys:ty,)*
            ),
            $cconv: ty,
            $cconv_lit: literal
        ) => {
            by_stack!(
                $num_args,
                $id_tys,
                $tuple_ids,
                ($($tail_args: $tail_tys,)*),
                $cconv,
                $cconv_lit
            );
        }
    }

    variadic_generic!(thunk_impl!(crate::cc::Win64, "win64"));
    variadic_generic!(thunk_impl!(crate::cc::Win64Unwind, "win64-unwind"));
}
