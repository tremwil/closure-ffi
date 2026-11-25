unsafe extern "win64-unwind" fn thunk_dispatcher<F>(
    // register params
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    // rsp+0x28 closure ptr
    ptr: *const F,
    // rsp+0x30 ret addr
    _ret: usize,
    // rsp+0x38 - rsp+0x50 original shadow slots 1-4
    _s0: usize,
    _s1: usize,
    _s2: usize,
    _s3: usize,
    // original 5th+ arguments
    a4: u64,
) -> f64
where
    F: Fn(u64, u64, u64, u64, u64) -> f64,
{
    unsafe { (*ptr)(a0, a1, a2, a3, a4) }
}

#[unsafe(naked)]
#[unsafe(link_section = ".text")]
unsafe extern "win64-unwind" fn thunk<F>(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> f64
where
    F: Fn(u64, u64, u64, u64, u64) -> f64,
{
    // convention: closure ptr in rax
    core::arch::naked_asm!(
        // todo: use ELF CFI directives on unix
        ".seh_proc {self_name}",
        // we need the return address (rsp) to now be in the 6th param position.
        "sub rsp, 0x28",
        ".seh_stackalloc 0x28",
        ".seh_endprologue",
        "mov [rsp+0x20], rax",
        "call {dispatcher}",
        "add rsp, 0x28",
        "ret",
        ".seh_endproc",
        self_name = sym thunk::<F>,
        dispatcher = sym thunk_dispatcher::<F>,
    )
}

type F = fn(u64, u64, u64, u64, u64) -> f64;

static THE_FN: F = |_, _, a, _, _| panic!("{a}") as f64;

#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "win64-unwind" fn jitted(a0: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> f64 {
    core::arch::naked_asm!(
        "lea rax, [rip + {}]",
        "jmp {}",
        sym THE_FN,
        sym thunk::<F>,
    )
}

fn main() {
    let n = unsafe { jitted(0, 1, 2, 3, 4) };
    println!("{n}");
}
