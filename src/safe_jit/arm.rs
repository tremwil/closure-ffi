use capstone::{
    arch::{
        arm::{ArchMode, ArmInsn, ArmOperandType, ArmReg},
        BuildsCapstone, DetailsArchInsn,
    },
    Capstone, InsnGroupId, InsnGroupType,
};

use super::{JitError, RelocThunk};

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

#[cfg(thumb_mode)]
const MODE: ArchMode = ArchMode::Thumb;
#[cfg(not(thumb_mode))]
const MODE: ArchMode = ArchMode::Arm;

pub fn try_reloc_thunk_template<'a>(
    thunk_template: &'a [u8],
    pc: u64,
    magic_offset: usize,
) -> Result<RelocThunk<'a>, JitError> {
    let thunk_template_end = thunk_template.len() as u64 + pc;
    let cs = Capstone::new().arm().mode(MODE).detail(true).build().unwrap();
    let mut disasm_iter = cs.disasm_iter(thunk_template, pc).unwrap();

    let mut has_thunk_asm = false;

    while let Some(instr) = disasm_iter.next() {
        let instr_pc = instr.address();

        let detail = cs.insn_detail(&instr).unwrap();
        let arch_detail = detail.arch_detail();
        let arm_detail = arch_detail.arm().unwrap();

        // Regular branch
        if instr.id().0 == ArmInsn::ARM_INS_B as u32 {
            let ArmOperandType::Imm(target) = arm_detail.operands().nth(0).unwrap().op_type
            else {
                unreachable!()
            };
            let target = target as u64;
            if (pc..thunk_template_end).contains(&target) {
                let offset = (target - pc) as usize;
                disasm_iter.reset(&thunk_template[offset..], target);
                continue;
            }
        }
        else if instr.id().0 == ArmInsn::ARM_INS_LDR as u32 {
            match arm_detail.operands().nth(1).unwrap().op_type {
                ArmOperandType::Mem(mem) if mem.base().0 as u32 == ArmReg::ARM_REG_PC => {
                    let pc_value = (instr_pc + 2 * instr.bytes().len() as u64) & !3;
                    let target = pc_value.wrapping_add_signed(mem.disp() as i64);

                    if target == pc + magic_offset as u64 {
                        has_thunk_asm = true;
                        break;
                    }
                }
                _ => (),
            }
        }

        if detail
            .regs_read()
            .iter()
            .chain(detail.regs_write())
            .any(|r| r.0 as u32 == ArmReg::ARM_REG_PC)
        {
            return Err(JitError::UnsupportedControlFlow);
        }

        if is_unsupported(detail.groups()) {
            return Err(JitError::UnsupportedControlFlow);
        }
    }

    if !has_thunk_asm {
        return Err(JitError::NoThunkAsm);
    }

    Ok(RelocThunk {
        thunk: thunk_template.into(),
        magic_offset,
    })
}
