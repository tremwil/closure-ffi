//! For ARM (A1) encoding, we need the following information for emitting/modifying
//! instructions:
//!
//! - Ability to emit a LDR, reg [pc, offset]
//! - Ability to turn common PC-relative loads into reg-relative loads.
//! - Ability to turn the ADR instruction into a LDR.

use capstone::arch::arm::ArmInsn;

use crate::safe_jit::arm_util::{bitflags, EncodingError};

pub const PC_OFFSET: usize = 8;

/// Return the value of PC as will be read by the CPU according to the address of the current
/// instruction ("actual" program counter).
fn effective_pc(instr_pc: usize) -> usize {
    (instr_pc + PC_OFFSET) & !3
}

bitflags! {
    /// Unconditional LDR instruction with no writeback.
    ///
    /// Reference: ARMv7 manual A8.8.65
    pub struct LdrImm: u32 {
        imm set_imm try_set_imm: 0..12,
        rt set_rt try_set_rt: 12..16,
        rn set_rn try_set_rn: 16..20,
        fixed0 set_fixed0: 20..23,
        u set_u: 23..24,
        fixed1 set_fixed1: 24..32
    }
}

impl LdrImm {
    #[allow(unused)]
    pub fn new_lit(pc: usize, reg: u32, target: usize) -> Result<Self, EncodingError> {
        let diff = target as isize - effective_pc(pc) as isize;
        Self::new(reg, 0b1111, diff.try_into().map_err(|_| ())?)
    }

    pub fn new(rt: u32, rn: u32, imm: i32) -> Result<Self, EncodingError> {
        let mut ins = Self::from_raw(0);
        ins.try_set_imm(imm.unsigned_abs())?;
        ins.try_set_rt(rt)?;
        ins.try_set_rn(rn)?;
        ins.set_fixed0(0b001);
        ins.set_u(if imm >= 0 { 1 } else { 0 });
        ins.set_fixed1(0b11100101);
        Ok(ins)
    }

    pub fn bytes(self) -> [u8; 4] {
        self.to_raw().to_ne_bytes()
    }
}

bitflags! {
    /// Operands for generic load word or byte immediate
    ///
    /// See ARMv7 manual, section A5.3.
    struct LoadWBImm: u32 {
        imm set_imm: 0..12,
        rt set_rt: 12..16,
        rn set_rn: 16..20,
        u set_u: 23..24
    }

    /// Operands for extra load instructions with immediates
    ///
    /// See ARMv7 manual, section A5.2.8.
    struct LoadExImm: u32 {
        imm_lo set_imm_lo: 0..4,
        imm_hi set_imm_hi: 8..12,
        rt set_rt: 12..16,
        rn set_rn: 16..20,
        u set_u: 23..24
    }
}

/// Generic load immediate instruction
#[allow(private_interfaces)]
#[derive(Debug, Clone, Copy)]
pub enum LoadImm {
    WB(LoadWBImm),
    Ex(LoadExImm),
}

impl LoadImm {
    pub fn try_from_raw(instr_id: ArmInsn, bytes: &[u8]) -> Option<Self> {
        use ArmInsn::*;

        let raw = u32::from_ne_bytes(bytes.try_into().ok()?);
        match instr_id {
            ARM_INS_LDR | ARM_INS_LDRT | ARM_INS_LDRB | ARM_INS_LDRBT => {
                Some(Self::WB(LoadWBImm::from_raw(raw)))
            }
            ARM_INS_LDRH | ARM_INS_LDRD | ARM_INS_LDRSB | ARM_INS_LDRSH | ARM_INS_LDRHT
            | ARM_INS_LDRSBT | ARM_INS_LDRSHT => Some(Self::Ex(LoadExImm::from_raw(raw))),
            _ => None,
        }
    }

    pub fn rt(&self) -> u32 {
        match self {
            Self::WB(ins) => ins.rt(),
            Self::Ex(ins) => ins.rt(),
        }
    }

    pub fn target_pc(&self, instr_pc: usize) -> Option<usize> {
        let pc = effective_pc(instr_pc);
        let (u, imm) = match self {
            Self::WB(ins) if ins.rn() == 0b1111 => (ins.u(), ins.imm()),
            Self::Ex(ins) if ins.rn() == 0b1111 => (ins.u(), ins.imm_hi() << 4 | ins.imm_lo()),
            _ => return None,
        };
        Some(if u == 1 { pc + imm as usize } else { pc - imm as usize })
    }

    /// Transform the load instruction into an equivalent one where the base register (Rn) is set to
    /// the target register (Rt) and the immediate is zeroed.
    pub fn as_rt_load<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        match self {
            Self::WB(mut ins) => {
                ins.set_rn(self.rt());
                ins.set_imm(0);

                f(&ins.to_raw().to_ne_bytes())
            }
            Self::Ex(mut ins) => {
                ins.set_rn(self.rt());
                ins.set_imm_lo(0);
                ins.set_imm_hi(0);

                f(&ins.to_raw().to_ne_bytes())
            }
        }
    }
}

bitflags! {
    /// ARMv7 ADR instruction, A1/A2 encodings.
    ///
    /// See ARMv7 manual, section A8.8.12 as well as A5-199 (ARMExpandImm).
    pub struct Adr: u32 {
        imm: 0..8,
        rot: 8..12,
        rd: 12..16,
        pc: 16..20,
        fixed0: 20..22,
        sign: 22..24,
        fixed1: 24..32
    }
}

impl Adr {
    pub fn try_from_raw(bytes: &[u8]) -> Option<Self> {
        let raw = u32::from_ne_bytes(bytes.try_into().ok()?);
        let ins = Adr::from_raw(raw);

        let is_ok = matches!(ins.sign(), 0b01 | 0b10)
            && ins.pc() == 0b1111
            && ins.fixed0() == 0
            && ins.fixed1() == 0b11100010;

        is_ok.then_some(ins)
    }

    pub fn dest_reg(&self) -> u32 {
        self.rd()
    }

    pub fn target_pc(&self, instr_pc: usize) -> usize {
        let disp = self.imm().rotate_right(2 * self.rot()) as usize;
        if self.sign() == 0b10 {
            (instr_pc + PC_OFFSET).wrapping_add(disp)
        }
        else {
            (instr_pc + PC_OFFSET).wrapping_sub(disp)
        }
    }
}
