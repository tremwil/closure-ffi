//! For Thumb encoding, we need the following information for emitting/modifying
//! instructions:
//!
//! - Ability to emit a LDR, reg [pc, offset]
//! - Ability to turn common PC-relative loads into reg-relative loads.
//! - Ability to turn the ADR instruction into a LDR.

use capstone::arch::arm::ArmInsn;

use crate::safe_jit::arm_util::{bitflags, EncodingError};

pub const PC_OFFSET: usize = 4;

/// Return the value of PC as will be read by the CPU according to the address of the current
/// instruction ("actual" program counter).
fn effective_pc(instr_pc: usize) -> usize {
    (instr_pc + PC_OFFSET) & !3
}

/// This module's u32 bitfield orderings are for little endian.
///
/// Since Thumb uses 16-bit instructions, the halfwords of a wide instruction must be swapped before
/// parsing.
fn flip16_if_be(raw: u32) -> u32 {
    #[cfg(target_endian = "big")]
    return (raw & 0xFFFF) << 16 | raw >> 16;
    #[cfg(target_endian = "little")]
    return raw;
}

bitflags! {
    /// Unconditional LDR.W instruction with no writeback (T3 imm encoding, T2 lit encoding).
    ///
    /// See ARMv7 manual, section A8.8.63.
    pub struct LdrImm: u32 {
        // block 1
        rn set_rn try_set_rn: 0..4,
        fixed0 set_fixed0: 4..7,
        u set_u: 7..8, // For PC only
        fixed1 set_fixed1: 8..16,
        // block 2
        imm set_imm try_set_imm: 16..28,
        rt set_rt try_set_rt: 28..32,
    }
}

impl LdrImm {
    #[allow(unused)]
    pub fn new_lit(pc: usize, reg: u32, target: usize) -> Result<Self, EncodingError> {
        let diff = target as isize - effective_pc(pc) as isize;
        Self::new(reg, 0b1111, diff.try_into().map_err(|_| ())?)
    }

    pub fn new(rt: u32, rn: u32, imm: i32) -> Result<Self, EncodingError> {
        if rn != 0b1111 && imm < 0 {
            return Err(EncodingError);
        }

        let mut ins = Self::from_raw(0);
        ins.try_set_imm(imm.unsigned_abs())?;
        ins.try_set_rt(rt)?;
        ins.try_set_rn(rn)?;
        ins.set_fixed0(0b101);
        ins.set_u(if imm >= 0 { 1 } else { 0 });
        ins.set_fixed1(0b11111000);
        Ok(ins)
    }

    pub fn bytes(self) -> [u8; 4] {
        flip16_if_be(self.to_raw()).to_ne_bytes()
    }
}

bitflags! {
    /// T1 encoded LDR (literal)
    ///
    /// See ARMv7 manual, section A8.8.63
    struct LdrLitT1: u16 {
        imm set_imm: 0..8,
        rt set_rt: 8..11,
        fixed: 11..16
    }

    /// T1 encoded LDR (immediate)
    ///
    /// See ARMv7 manual, section A8.8.63
    struct LdrImmT1: u16 {
        rt set_rt: 0..3,
        rn set_rn: 3..6,
        imm set_imm: 6..11,
        fixed set_fixed: 11..16
    }

    /// T3 encoded LDRxx (immediate). Also includes T2 LDR (literal) encoding with sign bit
    ///
    /// See ARMv7 manual, section A8.8.63
    struct LdrW: u32 {
        // block 1
        rn set_rn: 0..4,
        u set_u: 7..8,
        // block 2
        imm set_imm: 16..28,
        rt set_rt: 28..32
    }

    /// T1 encoded LDRxx instruction with short immediate/literal offset.
    ///
    /// Basically LDRD and T variants of LDR instructions.
    ///
    /// Note that `u` is only for LDRD.
    ///
    /// See ARMv7 manual, section A8.8.74
    struct LdrShortImm: u32 {
        // block 1
        rn set_rn: 0..4,
        u set_u: 7..8,
        // block 2
        imm set_imm: 16..24,
        rt set_rt: 28..32
    }
}

/// Generic load immediate/literal instructions
#[allow(private_interfaces)]
#[derive(Debug, Clone, Copy)]
pub enum LoadImm {
    LdrLitT1(LdrLitT1),
    LdrW(LdrW),
    LdrT(LdrShortImm),
    LdrD(LdrShortImm),
}

impl LoadImm {
    pub fn try_from_raw(instr_id: ArmInsn, bytes: &[u8]) -> Option<Self> {
        use ArmInsn::*;

        if bytes.len() == 2 {
            let raw = LdrLitT1::from_raw(u16::from_ne_bytes(bytes.try_into().unwrap()));
            (raw.fixed() == 0b01001).then_some(Self::LdrLitT1(raw))
        }
        else {
            let raw = flip16_if_be(u32::from_ne_bytes(bytes.try_into().ok()?));
            match instr_id {
                ARM_INS_LDR | ARM_INS_LDRB | ARM_INS_LDRH | ARM_INS_LDRSB | ARM_INS_LDRSH => {
                    Some(Self::LdrW(LdrW::from_raw(raw)))
                }
                ARM_INS_LDRT | ARM_INS_LDRBT | ARM_INS_LDRHT | ARM_INS_LDRSBT | ARM_INS_LDRSHT => {
                    Some(Self::LdrT(LdrShortImm::from_raw(raw)))
                }
                ARM_INS_LDRD => Some(Self::LdrD(LdrShortImm::from_raw(raw))),

                _ => None,
            }
        }
    }

    pub fn rt(&self) -> u32 {
        match self {
            Self::LdrLitT1(ldr) => ldr.rt() as u32,
            Self::LdrW(ldr) => ldr.rt(),
            Self::LdrT(ldr) | Self::LdrD(ldr) => ldr.rt(),
        }
    }

    pub fn target_pc(&self, instr_pc: usize) -> Option<usize> {
        let pc = effective_pc(instr_pc);
        match self {
            Self::LdrLitT1(ldr) => Some(pc + ldr.imm() as usize * 4),
            Self::LdrW(ldr) if ldr.rn() == 0b1111 => Some(pc + ldr.imm() as usize),
            Self::LdrT(ldr) if ldr.rn() == 0b1111 => Some(pc + ldr.imm() as usize),
            Self::LdrD(ldr) if ldr.rn() == 0b1111 => {
                let imm = if ldr.u() == 1 { ldr.imm() as isize } else { -(ldr.imm() as isize) };
                Some(pc.wrapping_sub_signed(imm * 4))
            }
            _ => None,
        }
    }

    /// Transform the load instruction into an equivalent one where the base register (Rn) is set to
    /// the target register (Rt) and the immediate is zeroed.
    pub fn as_rt_load<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        match self {
            Self::LdrLitT1(ldr) => {
                let mut new = LdrImmT1::from_raw(0);
                new.set_rt(ldr.rt());
                new.set_rn(ldr.rt());
                new.set_fixed(0b01101);
                f(&new.to_raw().to_ne_bytes())
            }
            Self::LdrW(mut ldr) => {
                ldr.set_rn(ldr.rt());
                ldr.set_imm(0);
                ldr.set_u(1);
                f(&flip16_if_be(ldr.to_raw()).to_ne_bytes())
            }
            Self::LdrT(mut ldr) | Self::LdrD(mut ldr) => {
                ldr.set_rn(ldr.rt());
                ldr.set_imm(0);

                if matches!(self, Self::LdrD(_)) {
                    ldr.set_u(1);
                }
                f(&flip16_if_be(ldr.to_raw()).to_ne_bytes())
            }
        }
    }
}

bitflags! {
    struct AdrT1: u16 {
        imm8: 0..8,
        rd: 8..11,
        fixed: 11..16
    }

    struct AdrT23: u32 {
        s1: 5..6,
        s2: 7..8,
        i1: 10..11,
        imm8: 16..24,
        rd: 24..28,
        imm3: 28..31
    }
}

#[allow(private_interfaces)]
#[derive(Debug)]
pub enum Adr {
    T1(AdrT1),
    T23(AdrT23),
}

impl Adr {
    pub fn try_from_raw(bytes: &[u8]) -> Option<Self> {
        if bytes.len() == 2 {
            let ins = AdrT1::from_raw(u16::from_ne_bytes(bytes.try_into().unwrap()));
            (ins.fixed() == 0b10100).then_some(Self::T1(ins))
        }
        else {
            let raw = flip16_if_be(u32::from_ne_bytes(bytes.try_into().ok()?));

            // check if the fixed bits are equal using two masks
            const MASK1: u32 = 0b0000_0000_0000_0000_1111_0010_0000_1111;
            const MASK0: u32 = 0b0111_1111_1111_1111_1111_0110_1010_1111;
            if raw & MASK1 != MASK1 || (raw | MASK0) != MASK0 {
                return None;
            }

            let ins = AdrT23::from_raw(raw);
            (ins.s1() != ins.s2()).then_some(Self::T23(ins))
        }
    }

    pub fn dest_reg(&self) -> u32 {
        match self {
            Self::T1(t1) => t1.rd() as u32,
            Self::T23(t23) => t23.rd(),
        }
    }

    pub fn target_pc(&self, instr_pc: usize) -> usize {
        let pc = effective_pc(instr_pc);

        match self {
            Self::T1(t1) => pc + t1.imm8() as usize * 4,
            Self::T23(t23) => {
                let full_imm = (t23.i1() << 11 | t23.imm3() << 8 | t23.imm8()) as usize;
                if t23.s1() == 0 {
                    pc + full_imm
                }
                else {
                    pc - full_imm
                }
            }
        }
    }
}
