use core::{
    alloc::Layout,
    arch::naked_asm,
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
};

#[repr(C)]
pub struct Thunk {
    // ldr xN, {closure}
    ldr_closure: u32,
    // ldr x9, {thunk}
    ldr_thunk: u32,
    // br x9
    br_thunk: u32,
    // .balign 8
    pad: u32,
    // .8byte closure
    closure: *const (),
    // .8byte thunk
    thunk: *const (),
}

pub fn next_avail_gpr<R, A, B, C, D, E, F, G, H>() -> Option<usize> {
    extern "C" fn setter<R, A, B, C, D, E, F, G, H>(
        _: MaybeUninit<A>,
        _: MaybeUninit<B>,
        _: MaybeUninit<C>,
        _: MaybeUninit<D>,
        _: MaybeUninit<E>,
        _: MaybeUninit<F>,
        _: MaybeUninit<G>,
        _: MaybeUninit<H>,
        gpr: usize,
        out: &mut usize,
    ) -> MaybeUninit<R> {
        *out = gpr;
        MaybeUninit::uninit()
    }

    #[unsafe(naked)]
    extern "C" fn probe<R, A, B, C, D, E, F, G, H>(
        _: MaybeUninit<A>,
        _: MaybeUninit<B>,
        _: MaybeUninit<C>,
        _: MaybeUninit<D>,
        _: MaybeUninit<E>,
        _: MaybeUninit<F>,
        _: MaybeUninit<G>,
        _: MaybeUninit<H>,
        gpr: usize,
        out: &mut usize,
    ) -> MaybeUninit<R> {
        naked_asm!(
            "cbnz x0, 1f",
            "mov x0, #1",
            "1:",
            "cbnz x1, 2f",
            "mov x1, #2",
            "2:",
            "cbnz x2, 3f",
            "mov x2, #3",
            "3:",
            "cbnz x3, 4f",
            "mov x3, #4",
            "4:",
            "cbnz x4, 5f",
            "mov x4, #5",
            "5:",
            "cbnz x5, 6f",
            "mov x5, #6",
            "6:",
            "cbnz x6, 7f",
            "mov x6, #7",
            "7:",
            "cbnz x7, 8f",
            "mov x7, #8",
            "8:",
            "b {}",
            sym setter::<R, A, B, C, D, E, F, G, H>
        )
    }

    let mut result = 0;
    probe::<R, A, B, C, D, E, F, G, H>(
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        0,
        &mut result,
    );

    (result > 0).then(|| result - 1)
}

struct T<'a, 'b: 'a, U> {
    p: PhantomData<&'a mut &'b mut U>,
}

type Test<'b, U> = for<'a> fn(&'a T<'a, 'b, U>);

type TestSet<'b, U> = fn(T<'static, 'b, U>);

struct WithLt<'a, U> {
    p: PhantomData<&'a mut U>,
}

fn generic<'a, U>(x: WithLt<'a, U>) -> WithLt<'a, U> {
    panic!()
}

fn generic2<U>(x: WithLt<'static, U>) -> WithLt<'static, U> {
    panic!()
}

use std::arch::asm;
use std::arch::naked_asm;
use std::mem::MaybeUninit;

#[derive(Debug)]
#[allow(unused)]
enum ArgSlot {
    Reg(usize),
    Stack(usize),
}

// simple upper bound for the amount of stack a function taking n arguments
// may use.
const fn max_arg_stack_use(n: usize) -> usize {
    // the argument that can occupy the most stack space is a 4-element HFA/HVA
    // of 128-bit fundamental types, which has 64 bytes.
    //
    // the argument that can fill up the most registers is also a 4-element HFA/HVA,
    // using up 4 floating point registers.
    //
    // Hence we need at least 2 args to use all registers and spill everything on
    // the stack.
    n.saturating_sub(2) * 64
}

fn next_arg_slot<R, A, B, C, D, E, F, G, H>() -> ArgSlot {
    const MAX_STACK: usize = max_arg_stack_use(8);

    extern "C" fn setter<R, A, B, C, D, E, F, G, H>(
        _: MaybeUninit<A>,
        _: MaybeUninit<B>,
        _: MaybeUninit<C>,
        _: MaybeUninit<D>,
        _: MaybeUninit<E>,
        _: MaybeUninit<F>,
        _: MaybeUninit<G>,
        _: MaybeUninit<H>,
        arg: usize,
        out: &mut usize,
    ) -> MaybeUninit<R> {
        *out = arg;
        MaybeUninit::uninit()
    }

    #[unsafe(naked)]
    extern "C" fn probe<R, A, B, C, D, E, F, G, H>(
        _: MaybeUninit<A>,
        _: MaybeUninit<B>,
        _: MaybeUninit<C>,
        _: MaybeUninit<D>,
        _: MaybeUninit<E>,
        _: MaybeUninit<F>,
        _: MaybeUninit<G>,
        _: MaybeUninit<H>,
        gpr: usize,
        out: &mut usize,
        _stack: &mut MaybeUninit<[u8; MAX_STACK]>,
    ) -> MaybeUninit<R> {
        naked_asm!(
            // probe registers (x0 is implicit)
            "cbnz x1, 1f",
            "mov x1, #1",
            "1:",
            "cbnz x2, 2f",
            "mov x2, #2",
            "2:",
            "cbnz x3, 3f",
            "mov x3, #3",
            "3:",
            "cbnz x4, 4f",
            "mov x4, #4",
            "4:",
            "cbnz x5, 5f",
            "mov x5, #5",
            "5:",
            "cbnz x6, 6f",
            "mov x6, #6",
            "6:",
            "cbnz x7, 7f",
            "mov x7, #7",
            "7:",
            // allocate a new stack frame, saving og sp in x9
            "mov x9, sp",
            "stp x29, x30, [sp, #-64]!",
            "mov x29, sp",
            "sub sp, sp, #{stack_probe}",
            // shift stack down, setting zero entries to offset
            "mov x10, #{stack_probe}",
            "cbz x10, 10f",
            "8:",
            "sub x10, x10, #8",
            "ldr x11, [x9, x10]",
            "cbnz x11, 9f",
            "add x11, x10, #8",
            "9:",
            "str x11, [sp, x10]",
            "cbnz x10, 8b",
            "10:",
            "bl {setter}",
            // pop stack frame
            "add sp, sp, #{stack_probe}",
            "ldp x29, x30, [sp], #64",
            "ret",
            stack_probe = const { MAX_STACK },
            setter = sym setter::<R, A, B, C, D, E, F, G, H>
        )
    }

    // Ensure we allocate at least as much stack as can
    // be read by the probe.
    let mut stack = MaybeUninit::uninit();
    let mut result = 0;

    probe::<R, A, B, C, D, E, F, G, H>(
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        0,
        &mut result,
        // this is not actually accessed, but since the naked function is
        // opaque to rustc it prevents the stack allocation from
        // being optimized away
        &mut stack,
    );

    match result {
        ..8 => ArgSlot::Reg(result),
        _ => ArgSlot::Stack(result - 8),
    }
}

#[unsafe(no_mangle)]
fn test_stack_48() -> ArgSlot {
    next_arg_slot::<
        // ret
        u64,
        // all grps
        [u64; 2],
        [u64; 2],
        [u64; 2],
        [u64; 2],
        // all fp regs
        [f32; 4],
        [f32; 4],
        // on stack
        [f64; 4], // 32 bytes
        [f32; 3], // 12 bytes
    >()
}

#[unsafe(no_mangle)]
fn test_reg_x6() -> ArgSlot {
    next_arg_slot::<
        // ret
        u64,
        // gprs 0-5 (inclusive)
        [u64; 2],
        [u64; 2],
        [u64; 3], // by pointer
        [u64; 4], // by pointer
        // all fp regs
        [f32; 4],
        [f32; 4],
        // on stack
        [f64; 4], // 32 bytes
        [f64; 3], // 32 bytes
    >()
}

#[unsafe(no_mangle)]
fn test_reg_x0() -> ArgSlot {
    next_arg_slot::<
        // ret
        u64,
        // all fp regs
        [f64; 4],
        [f64; 4],
        // on stack (6x32 = 196 bytes)
        // note: rust doesn't seem to handle HVAs properly, it treats small
        // SIMD vectors as non-fundamental and therefore a 2-element HFA.
        [f64; 4],
        [f64; 4],
        [f64; 4],
        [f64; 4],
        [f64; 4],
        [f64; 4],
    >()
}

fn main() {
    println!("{:?}", test_stack_48());
    println!("{:?}", test_reg_x6());
    println!("{:?}", test_reg_x0());
}
