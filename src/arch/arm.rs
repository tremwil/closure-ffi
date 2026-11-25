#[repr(C)]
pub struct ThunkA32 {
    // ldr rN, {closure}
    ldr_closure: u32,
    // ldr r12, {thunk}
    ldr_thunk: u32,
    // bx r12
    bx_thunk: u32,
    // .4byte closure
    closure: *const (),
    // .4byte thunk
    thunk: *const (),
}

#[repr(C)]
pub struct ThunkT32 {
    // ldr rN, {closure}
    ldr_closure: u16,
    // ldr r12, {thunk}
    ldr_thunk: u16,
    // bx r12
    bx_thunk: u16,
    // .balign 4
    _pad: u16,
    // .4byte closure
    closure: *const (),
    // .4byte thunk
    thunk: *const (),
}
