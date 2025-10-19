use alloc::vec::Vec;

use capstone::{
    arch::{
        arm::{ArchMode, ArmCC, ArmInsn, ArmOperandType, ArmReg},
        BuildsCapstone, DetailsArchInsn,
    },
    Capstone,
};

use crate::safe_jit::{
    arm_util::{
        encoding::{Adr, LdrImm, LoadImm},
        has_unsupported_insn_group, CowBuffer,
    },
    JitError, RelocThunk,
};

#[cfg(thumb_mode)]
pub const MODE: ArchMode = ArchMode::Thumb;
#[cfg(not(thumb_mode))]
pub const MODE: ArchMode = ArchMode::Arm;

pub fn try_reloc_thunk_template<'a>(
    thunk_template: &'a [u8],
    pc: usize,
    magic_offset: usize,
) -> Result<RelocThunk<'a>, JitError> {
    let thunk_template_end = thunk_template.len() + pc;
    let cs = Capstone::new().arm().mode(MODE).detail(true).build().unwrap();
    let mut disasm_iter = cs.disasm_iter(thunk_template, pc as u64).unwrap();

    let mut has_thunk_asm = false;
    let mut new_magic_offset = magic_offset;
    let mut cow_buf = CowBuffer::new(thunk_template);
    let mut extra_ldrs = Vec::new();

    while let Some(instr) = disasm_iter.next() {
        let instr_pc = instr.address() as usize;
        let instr_id = ArmInsn::from(instr.id().0);
        let offset = instr_pc - pc;

        let detail = cs.insn_detail(&instr).unwrap();
        let arch_detail = detail.arch_detail();
        let arm_detail = arch_detail.arm().unwrap();

        // Regular branch (B)
        if instr_id == ArmInsn::ARM_INS_B {
            let ArmOperandType::Imm(target) = arm_detail.operands().next().unwrap().op_type
            else {
                unreachable!()
            };
            let target = target as usize;
            if (pc..thunk_template_end).contains(&target) {
                disasm_iter.reset(&thunk_template[target - pc..], target as u64);
                continue;
            }
        }
        // error on other instruction writing to PC or from unsupported groups
        else if detail.regs_write().iter().any(|r| r.0 as u32 == ArmReg::ARM_REG_PC)
            || has_unsupported_insn_group(detail.groups())
        {
            return Err(JitError::UnsupportedInstruction);
        }
        // if the instruction doesn't read the PC, we don't care about it at this point.
        // it's OK to relocate it.
        if !detail.regs_read().iter().any(|r| r.0 as u32 == ArmReg::ARM_REG_PC) {
            continue;
        }
        // We don't support relocating instructions that conditionally read the PC.
        if arm_detail.cc() != ArmCC::ARM_CC_AL {
            return Err(JitError::UnsupportedInstruction);
        }

        // ADR reg, label => LDR reg, =label_address
        if let Some(adr) = Adr::try_from_raw(instr.bytes()) {
            new_magic_offset += 4 - instr.len();

            // allocate space for a new LDR
            let ldr_ofs = cow_buf.append(offset, &[0; 4]);
            extra_ldrs.push((ldr_ofs, adr.dest_reg(), adr.target_pc(instr_pc)));

            // ignore the original ADR instruction
            cow_buf.ignore(offset, instr.len());
            continue;
        }
        // LDR.. reg, label => LDR reg, =label_address; LDR.. reg, [reg]
        else if let Some(load) = LoadImm::try_from_raw(instr_id, instr.bytes()) {
            if let Some(target) = load.target_pc(instr_pc) {
                if target == pc + magic_offset {
                    has_thunk_asm = true;
                    break;
                }

                // allocate space for a new LDR
                new_magic_offset += 4;
                let ldr_ofs = cow_buf.append(offset, &[0; 4]);
                extra_ldrs.push((ldr_ofs, load.rt(), target));

                // replace the instruction by a rt load (LDRxx reg, [reg])
                load.as_rt_load(|new| cow_buf.append(offset, new));
                cow_buf.ignore(offset, instr.len());
                continue;
            }
        }

        // any other pc-relative reads are unsupported
        return Err(JitError::UnsupportedInstruction);
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
        if !(new_bytes.len() - new_magic_offset).is_multiple_of(4) {
            new_bytes.extend_from_slice(&[0; 2]);
        }

        // write the absolute addresses to the literal pool and emit LDR instructions
        // referring to them.
        for (instr_offset, reg, addr) in extra_ldrs {
            let addr_pc = pc + new_bytes.len();
            let ldr = LdrImm::new_lit(pc + instr_offset, reg, addr_pc)?;

            new_bytes.extend_from_slice(&addr.to_ne_bytes());
            new_bytes[instr_offset..instr_offset + 4].copy_from_slice(&ldr.bytes());
        }
    }

    Ok(RelocThunk {
        thunk: cow_buf.into_bytes(),
        magic_offset: new_magic_offset,
    })
}
