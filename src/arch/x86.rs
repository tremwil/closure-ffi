#[repr(C, align(4))]
pub struct Thunk {
    // mov rN, {closure}
    mov_closure_reg: u8,
    closure: [u8; 4],
    // mov eax, {thunk}
    mov_thunk: u8,
    thunk: [u8; 4],
    // jmp eax,
    jmp_thunk: [u8; 2],
}

impl Thunk {}
