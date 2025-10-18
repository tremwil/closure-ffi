use alloc::vec::Vec;

use iced_x86::{
    Code, Decoder, DecoderOptions, Encoder, FlowControl, Instruction, InstructionInfoFactory,
    InstructionInfoOptions, OpAccess, Register,
};

use super::{JitError, RelocThunk};
use crate::arch::consts;

pub fn try_reloc_thunk_template<'a>(
    thunk_template: &'a [u8],
    ip: usize,
    magic_offset: usize,
) -> Result<RelocThunk<'a>, JitError> {
    let ip = ip as u64;

    let mut decoder = Decoder::with_ip(64, thunk_template, ip, DecoderOptions::NONE);

    let mut instructions = Vec::new();
    let mut instruction = Instruction::default();
    let mut num_ip_rel_reloc = 0;
    let mut thunk_asm_offset = None;

    while decoder.can_decode() {
        decoder.decode_out(&mut instruction);

        if instruction.is_invalid() {
            return Err(JitError::InvalidInstruction);
        }
        else if instruction.flow_control() != FlowControl::Next {
            return Err(JitError::UnsupportedInstruction);
        }

        let needs_reloc = instruction.is_ip_rel_memory_operand();
        if needs_reloc {
            let closure_ptr_offset = magic_offset.wrapping_add_signed(consts::CLOSURE_ADDR_OFFSET);
            if (instruction.memory_displacement64() - ip) as usize == closure_ptr_offset {
                thunk_asm_offset = Some(decoder.position() - instruction.len());
                break;
            }

            num_ip_rel_reloc += 1;
        }

        instructions.push((instruction, needs_reloc));
    }

    let thunk_asm_offset = thunk_asm_offset.ok_or(JitError::NoThunkAsm)?;
    if num_ip_rel_reloc == 0 {
        return Ok(RelocThunk {
            thunk: thunk_template.into(),
            magic_offset,
        });
    }

    // go through the instructions backwards and track the last visible read/write
    // operation on general purpose registers
    //
    // this lets us easily see which can be picked to load ip rel mem addresses

    #[derive(Clone, Copy, Eq, PartialEq)]
    enum GprOp {
        None,
        Read(usize),
        Write(usize),
    }

    // grab the register used to read the closure address the thunk asm
    decoder.set_position(thunk_asm_offset).unwrap();
    let cl_read_reg = decoder.decode().op0_register();

    let mut gpr_ops = [GprOp::None; 16];
    gpr_ops[cl_read_reg as usize - Register::RAX as usize] = GprOp::Write(instruction.len());

    let mut info_factory = InstructionInfoFactory::new();
    let mut chosen_registers = Vec::with_capacity(num_ip_rel_reloc);
    for (i, &(instr, needs_reloc)) in instructions.iter().enumerate().rev() {
        let info = info_factory.info_options(&instr, InstructionInfoOptions::NO_MEMORY_USAGE);
        for used_gpr in info.used_registers().iter().filter(|u| u.register().is_gpr()) {
            let full_register = used_gpr.register().full_register();
            let last_op = &mut gpr_ops[full_register as usize - Register::RAX as usize];

            if used_gpr.access() == OpAccess::Write {
                if used_gpr.register().size() >= 4 && *last_op != GprOp::Read(i) {
                    *last_op = GprOp::Write(i);
                }
            }
            else if used_gpr.access() != OpAccess::None {
                *last_op = GprOp::Read(i);
            }
        }

        if needs_reloc {
            // try to find a gpr that is fully clobbered by a write
            // we don't have to update the ops since the new read will be hidden by the address load
            let Some(i_avail_gpr) =
                gpr_ops.iter().position(|&op| matches!(op, GprOp::Write(w) if w >= i))
            else {
                return Err(JitError::NoAvailableRegister);
            };
            chosen_registers.push(Register::RAX + i_avail_gpr as u32)
        }
    }

    // now that we know which registers to use, re-encode the instructions

    const MOV_R64_IMM64_SIZE: usize = 10;
    let min_new_size = thunk_template.len() + MOV_R64_IMM64_SIZE * num_ip_rel_reloc;
    let mut encoder = Encoder::try_with_capacity(64, min_new_size).unwrap();

    let mut offset = 0;
    for (mut instr, needs_reloc) in instructions {
        if needs_reloc {
            // cannot fail as one was pushed for each instr that needs a reloc
            let register = chosen_registers.pop().unwrap();
            let address = instr.memory_displacement64();

            // cannot fail as register is a 64-bit gpr
            let mov = Instruction::with2(Code::Mov_r64_imm64, register, address).unwrap();
            encoder.encode(&mov, 0).map_err(|_| JitError::EncodingError)?;

            instr.set_memory_base(register);
            instr.set_memory_index(Register::None); // shouldn't be necessary
            instr.set_memory_displacement64(0);
            encoder.encode(&instr, 0).map_err(|_| JitError::EncodingError)?;
        }
        else {
            let mut buffer = encoder.take_buffer();
            buffer.extend_from_slice(&thunk_template[offset..offset + instr.len()]);
            encoder.set_buffer(buffer);
        }

        offset += instr.len();
    }

    // add the part that includes the thunk_asm block
    let mut new_bytes = encoder.take_buffer();
    new_bytes.extend_from_slice(&thunk_template[thunk_asm_offset..]);

    Ok(RelocThunk {
        magic_offset: magic_offset + new_bytes.len() - thunk_template.len(),
        thunk: new_bytes.into(),
    })
}
