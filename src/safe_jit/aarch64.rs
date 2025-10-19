use alloc::vec::Vec;

use capstone::{
    arch::{arm64::ArchMode, BuildsCapstone},
    Capstone,
};

use crate::safe_jit::{
    arm_util::{
        encoding::{Adr, Branch, LdrImm, LdrImmOpc, LdrOfs},
        has_unsupported_insn_group, CowBuffer,
    },
    JitError, RelocThunk,
};

pub fn try_reloc_thunk_template<'a>(
    thunk_template: &'a [u8],
    pc: usize,
    magic_offset: usize,
) -> Result<RelocThunk<'a>, JitError> {
    let thunk_template_end = thunk_template.len() + pc;
    let cs = Capstone::new().arm64().mode(ArchMode::Arm).detail(true).build().unwrap();
    let mut disasm_iter = cs.disasm_iter(thunk_template, pc as u64).unwrap();

    let mut new_magic_offset = magic_offset;
    let mut has_thunk_asm = false;

    let mut extra_ldrs = Vec::new();
    let mut cow_buf = CowBuffer::new(thunk_template);

    while let Some(instr) = disasm_iter.next() {
        let instr_pc = instr.address() as usize;
        let offset = instr_pc - pc;
        let instr_u32 = u32::from_ne_bytes(instr.bytes().try_into().unwrap());

        // LDR/LDRW/LDRSW reg, label
        // we have to turn the instruction into:
        // LDR reg, =abs_address
        // LDR reg, [reg]
        if let Ok(ldr) = LdrImm::try_from_raw(instr_u32) {
            let target = ldr.target_pc(instr_pc);
            if target == pc + magic_offset {
                has_thunk_asm = true;
                break;
            }

            // make space to insert the new LdrImm instruction
            let to_encode = cow_buf.append(offset, &[0; 4]);

            // push a future ldr at this offset
            extra_ldrs.push((to_encode, ldr.reg(), target));
            new_magic_offset += 4;

            // replace the original instruction with LDR reg, [reg]
            let ldr64 = LdrOfs::new(ldr.opc(), ldr.reg(), ldr.reg(), 0)?;
            cow_buf.replace(offset, &ldr64.to_raw().to_ne_bytes());
        }
        // ADR/ADRP reg, label
        // we have to turn the instruction into:
        // LDR reg, =abs_address
        else if let Ok(adr) = Adr::try_from_raw(instr_u32) {
            // replace the original instruction with free space for a future ldr there
            let to_encode = cow_buf.replace(offset, &[0; 4]);
            extra_ldrs.push((to_encode, adr.reg(), adr.target_pc(instr_pc)))
        }
        // B label
        else if let Ok(branch) = Branch::try_from_raw(instr_u32) {
            // follow forward branches that stay within the thunk
            let target = branch.target_pc(instr_pc);
            if (instr_pc + 4..thunk_template_end).contains(&target) {
                disasm_iter.reset(&thunk_template[target - pc..], target as u64);
            }
            else {
                return Err(JitError::UnsupportedInstruction);
            }
        }
        else if has_unsupported_insn_group(cs.insn_detail(&instr).unwrap().groups()) {
            return Err(JitError::UnsupportedInstruction);
        }
    }

    if !has_thunk_asm {
        return Err(JitError::NoThunkAsm);
    }

    // emit the extra LDR instructions using a post-thunk literal pool
    if !extra_ldrs.is_empty() {
        // copy the rest of the thunk template over
        cow_buf.copy_up_to(thunk_template.len());
        let new_bytes = cow_buf.new_bytes_mut();

        // the contract is that the magic offset is at least pointer-aligned.
        // use it to determine if we need to add padding to align the literals.
        if !(new_bytes.len() - new_magic_offset).is_multiple_of(8) {
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
        thunk: cow_buf.into_bytes(),
        magic_offset: new_magic_offset,
    })
}
