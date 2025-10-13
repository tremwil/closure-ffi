use alloc::vec::Vec;

use iced_x86::{Code, Decoder, DecoderOptions, Encoder, FlowControl, Instruction, Register};

use super::{JitError, RelocThunk};
use crate::arch::consts;

struct CallPop {
    offset: usize,
    len: usize,
    target_ip: u32,
    register: Register,
}

pub fn try_reloc_thunk_template<'a>(
    thunk_template: &'a [u8],
    ip: u64,
    magic_offset: usize,
) -> Result<RelocThunk<'a>, JitError> {
    let mut decoder = Decoder::with_ip(32, thunk_template, ip, DecoderOptions::NONE);

    let mut instruction = Instruction::default();
    let mut reached_thunk_asm = false;
    let mut call_pops = Vec::new();

    while decoder.can_decode() {
        let offset = decoder.position();
        if offset == magic_offset.wrapping_add_signed(consts::THUNK_CODE_OFFSET) {
            reached_thunk_asm = true;
            break;
        }

        decoder.decode_out(&mut instruction);
        if instruction.is_invalid() {
            return Err(JitError::InvalidInstruction);
        }

        if instruction.flow_control() != FlowControl::Next {
            // a pattern that can appear in prologues is CALL rip+0, POP reg.
            // we have to transform it into MOV reg, absolute_pc.
            if instruction.code() == Code::Call_rel32_32
                && instruction.near_branch32() == instruction.next_ip32()
            {
                let next_instruction = decoder.decode();
                if next_instruction.code() == Code::Pop_r32 {
                    call_pops.push(CallPop {
                        offset,
                        len: next_instruction.len() + instruction.len(),
                        target_ip: instruction.next_ip32(),
                        register: next_instruction.op0_register(),
                    });
                    continue;
                }
            }

            return Err(JitError::UnsupportedControlFlow);
        }
    }

    if !reached_thunk_asm {
        return Err(JitError::NoThunkAsm);
    }

    if call_pops.is_empty() {
        return Ok(RelocThunk {
            thunk: thunk_template.into(),
            magic_offset,
        });
    }

    // note: this is less than the minimum required by a call+pop (6 bytes), assuming no prefixes
    // thus the subtract should never underflow
    const MOV_R32_IMM32_LEN: usize = 5;
    let required_mem =
        thunk_template.len() - call_pops.iter().map(|c| c.len - MOV_R32_IMM32_LEN).sum::<usize>();

    let mut offset = 0;
    let mut encoder = Encoder::try_with_capacity(32, required_mem).unwrap();

    for call_pop in call_pops {
        // copy the instruction slice between the call pops
        let mut buf = encoder.take_buffer();
        buf.extend_from_slice(&thunk_template[offset..call_pop.offset]);
        encoder.set_buffer(buf);
        offset = call_pop.offset + call_pop.len;

        // can't panic (register is from a pop r32 and is thus a 32-bit gpr)
        let mov =
            Instruction::with2(Code::Mov_r32_imm32, call_pop.register, call_pop.target_ip).unwrap();
        encoder.encode(&mov, 0).map_err(|_| JitError::EncodingError)?;
    }

    // write the remaining slice
    let mut new_bytes = encoder.take_buffer();
    new_bytes.extend_from_slice(&thunk_template[offset..]);

    Ok(RelocThunk {
        magic_offset: magic_offset + new_bytes.len() - thunk_template.len(),
        thunk: new_bytes.into(),
    })
}
