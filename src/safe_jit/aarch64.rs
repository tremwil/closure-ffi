use alloc::{borrow::Cow, collections::vec_deque::VecDeque, string::String, vec::Vec};
use core::ops::RangeBounds;

use capstone::{
    arch::{
        arm64::{
            ArchMode,
            Arm64Insn::{self, *},
            Arm64Operand, Arm64OperandType, Arm64Reg,
        },
        BuildsCapstone, DetailsArchInsn,
    },
    Capstone, InsnGroupId, InsnGroupType,
};

use crate::safe_jit::{
    aarch64::encoding::{Branch, LdrImm, LdrImmOpc, LdrOfs},
    JitError, RelocThunk,
};

mod encoding;

use encoding::Adr;

// happens to be everything below 8 but it's better to implement this way and let the compiler
// optimize
const UNSUPPORTED_GROUPS: &[InsnGroupType::Type] = &[
    InsnGroupType::CS_GRP_INVALID,
    InsnGroupType::CS_GRP_JUMP,
    InsnGroupType::CS_GRP_CALL,
    InsnGroupType::CS_GRP_RET,
    InsnGroupType::CS_GRP_INT,
    InsnGroupType::CS_GRP_IRET,
    InsnGroupType::CS_GRP_PRIVILEGE,
    InsnGroupType::CS_GRP_BRANCH_RELATIVE,
];

fn is_unsupported(groups: &[InsnGroupId]) -> bool {
    groups
        .iter()
        .any(|g| UNSUPPORTED_GROUPS.contains(&(g.0 as InsnGroupType::Type)))
}

/// Lazy buffer + cursor for incrementally replacing and adding bytes to a slice
struct LazyRecoder<'a> {
    orig_bytes: &'a [u8],
    new_bytes: Vec<u8>,
    num_copied: usize,
}

impl<'a> LazyRecoder<'a> {
    fn new(orig_bytes: &'a [u8]) -> Self {
        Self {
            orig_bytes,
            new_bytes: Vec::new(),
            num_copied: 0,
        }
    }

    fn into_bytes(mut self) -> Cow<'a, [u8]> {
        if self.new_bytes.is_empty() {
            self.orig_bytes.into()
        }
        else {
            // write the remaining slice
            self.new_bytes.extend_from_slice(&self.orig_bytes[self.num_copied..]);
            self.new_bytes.into()
        }
    }

    /// Get a reference to the new buffer.
    fn new_bytes(&self) -> &[u8] {
        &self.new_bytes
    }

    /// Get a mutable reference to the new buffer.
    ///
    /// You may grow this buffer arbitrarily, but shrinking it will lead to logic errors.
    fn new_bytes_mut(&mut self) -> &mut Vec<u8> {
        &mut self.new_bytes
    }

    /// Copy bytes from the original slice into [`Self::new_bytes`] up to `offset` in the original
    /// slice.
    fn copy_up_to(&mut self, offset: usize) {
        debug_assert!(offset >= self.num_copied && offset <= self.orig_bytes.len());

        // preallocate minimum capacity
        if self.new_bytes.is_empty() {
            self.new_bytes = Vec::with_capacity(self.orig_bytes.len())
        }
        let new_num_copied = offset.min(self.orig_bytes.len());
        self.new_bytes
            .extend_from_slice(&self.orig_bytes[self.num_copied..new_num_copied]);
        self.num_copied = new_num_copied;
    }

    /// Insert new bytes in the buffer at after an offset in the original slice.
    ///
    /// Returns the offset of `bytes` in [`Self::new_bytes`].
    fn append(&mut self, offset: usize, bytes: &[u8]) -> usize {
        if !bytes.is_empty() {
            self.copy_up_to(offset);
        }
        let bytes_offset = self.new_bytes.len();
        self.new_bytes.extend_from_slice(bytes);
        return bytes_offset;
    }

    /// Replace the bytes at `offset` in the original slice.
    ///
    /// Returns the offset of `bytes` in [`Self::new_bytes`].
    fn replace(&mut self, offset: usize, bytes: &[u8]) -> usize {
        debug_assert!(offset + bytes.len() <= self.orig_bytes.len());

        let bytes_offset = self.append(offset, bytes);
        self.num_copied += bytes.len();
        bytes_offset
    }
}

pub fn try_reloc_thunk_template<'a>(
    thunk_template: &'a [u8],
    pc: u64,
    magic_offset: usize,
) -> Result<RelocThunk<'a>, JitError> {
    let thunk_template_end = thunk_template.len() as u64 + pc;
    let cs = Capstone::new().arm64().mode(ArchMode::Arm).detail(true).build().unwrap();
    let mut disasm_iter = cs.disasm_iter(thunk_template, pc).unwrap();

    let mut magic_offset_shift = 0;
    let mut has_thunk_asm = false;

    let mut extra_ldrs = Vec::new();
    let mut recoder = LazyRecoder::new(thunk_template);

    while let Some(instr) = disasm_iter.next() {
        let instr_pc = instr.address();
        let offset = (instr_pc - pc) as usize;
        let instr_u32 = u32::from_ne_bytes(instr.bytes().try_into().unwrap());

        // LDR/LDRW/LDRSW/PRFM reg, label
        // we have to turn the instruction into:
        // LDR reg, =abs_address
        // LDR reg, [reg]
        if let Ok(ldr) = LdrImm::try_from_raw(instr_u32) {
            let target = ldr.target_pc(instr_pc);
            if target == pc + magic_offset as u64 {
                has_thunk_asm = true;
                break;
            }

            // make space to insert the new LdrImm instruction
            let to_encode = recoder.append(offset, &[0; 4]);

            // push a future ldr at this offset
            extra_ldrs.push((to_encode, ldr.reg(), target));
            magic_offset_shift += 4;

            // replace the original instruction with LDR reg, [reg]
            let ldr64 = LdrOfs::new(ldr.opc(), ldr.reg(), ldr.reg(), 0)?;
            recoder.replace(offset, &ldr64.to_raw().to_ne_bytes());
        }
        // ADR/ADRP reg, label
        // we have to turn the instruction into:
        // LDR reg, =abs_address
        else if let Ok(adr) = Adr::try_from_raw(instr_u32) {
            // replace the original instruction with free space for a future ldr there
            let to_encode = recoder.replace(offset, &[0; 4]);
            extra_ldrs.push((to_encode, adr.reg(), adr.target_pc(instr_pc)))
        }
        // B label
        else if let Ok(branch) = Branch::try_from_raw(instr_u32) {
            // follow forward branches that stay within the thunk
            let target = branch.target_pc(instr_pc);
            if (instr_pc + 4..thunk_template_end).contains(&target) {
                let new_offset = (target - pc) as usize;
                disasm_iter.reset(&thunk_template[new_offset..], target);
            }
            else {
                return Err(JitError::UnsupportedControlFlow);
            }
        }
        // non-fallthrough control flow (not supported)
        else if is_unsupported(cs.insn_detail(&instr).unwrap().groups()) {
            return Err(JitError::UnsupportedControlFlow);
        }
    }

    if !has_thunk_asm {
        return Err(JitError::NoThunkAsm);
    }

    // adjust the magic offset for the new buffer, if modified
    let magic_offset = magic_offset + magic_offset_shift;

    // emit the extra LDR instructions using a post-thunk literal pool
    if !extra_ldrs.is_empty() {
        // copy the rest of the thunk template over
        recoder.copy_up_to(thunk_template.len());
        let new_bytes = recoder.new_bytes_mut();

        // the contract is that the magic offset is at least pointer-aligned.
        // use it to determine if we need to add padding to align the literals.
        if !(new_bytes.len() - magic_offset).is_multiple_of(8) {
            new_bytes.extend_from_slice(&[0; 4]);
        }

        // write the absolute addresses to the literal pool and emit LDR instructions
        // referring to them.
        for (instr_offset, reg, addr) in extra_ldrs {
            let pc_offset = new_bytes.len() - instr_offset;
            let ldr = LdrImm::new(LdrImmOpc::Load64, reg, pc_offset as i32 / 4)?;
            let ldr_bytes = &ldr.to_raw().to_ne_bytes();

            new_bytes.extend_from_slice(&addr.to_ne_bytes());
            new_bytes[instr_offset..instr_offset + 4].copy_from_slice(ldr_bytes);
        }
    }

    Ok(RelocThunk {
        thunk: recoder.into_bytes(),
        magic_offset,
    })
}
