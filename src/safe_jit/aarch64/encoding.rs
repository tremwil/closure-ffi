use crate::safe_jit::{util::bitflags, JitError};

#[derive(Debug, Clone, Copy)]
pub struct Error;

impl From<()> for Error {
    fn from(_value: ()) -> Self {
        Error
    }
}

impl From<Error> for JitError {
    fn from(_value: Error) -> Self {
        JitError::EncodingError
    }
}

// https://developer.arm.com/documentation/ddi0602/2022-09/Base-Instructions/LDR--literal---Load-Register--literal--
bitflags! {
    pub struct LdrImm: u32 {
        pub reg set_reg try_set_reg: 0..5,
        #[signed(i32)]
        pub imm set_imm try_set_imm: 5..24,
        fixed set_fixed: 24..30,
        opc_raw set_opc_raw: 30..32
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LdrImmOpc {
    Load32,
    Load64,
    Load32Sx,
    Prefetch,
}

impl LdrImm {
    pub fn try_from_raw(raw: u32) -> Result<Self, ()> {
        let ins = Self::from_raw(raw);
        ins.assert_opcode().then_some(ins).ok_or(())
    }

    pub fn new_at(pc: u64, opc: LdrImmOpc, reg: u32, target: u64) -> Result<Self, Error> {
        let diff = target as i64 - pc as i64;
        if diff % 4 != 0 {
            return Err(Error);
        }
        Self::new(opc, reg, (diff / 4).try_into().map_err(|_| ())?)
    }

    pub fn assert_opcode(&self) -> bool {
        self.fixed() == 0b11000
    }

    pub fn new(opc: LdrImmOpc, reg: u32, imm: i32) -> Result<Self, Error> {
        let mut ins = Self::from_raw(0);
        ins.try_set_reg(reg)?;
        ins.try_set_imm(imm)?;
        ins.set_fixed(0b11000);
        ins.set_opc_raw(opc as u32);
        Ok(ins)
    }

    pub fn target_pc(&self, pc: u64) -> u64 {
        pc.wrapping_add_signed(self.imm() as i64 * 4)
    }

    pub fn opc(&self) -> LdrImmOpc {
        match self.opc_raw() {
            0 => LdrImmOpc::Load32,
            1 => LdrImmOpc::Load64,
            2 => LdrImmOpc::Load32Sx,
            3 => LdrImmOpc::Prefetch,
            _ => unreachable!(),
        }
    }
}

// https://developer.arm.com/documentation/ddi0602/2022-09/Base-Instructions/ADR--Form-PC-relative-address
bitflags! {
    pub struct Adr: u32 {
        pub reg: 0..5,
        imm_hi: 5..24,
        fixed: 24..29,
        imm_lo: 29..31,
        is_adrp_raw: 31..32
    }
}

impl Adr {
    pub fn try_from_raw(raw: u32) -> Result<Self, ()> {
        let ins = Self::from_raw(raw);
        ins.assert_opcode().then_some(ins).ok_or(())
    }

    pub fn assert_opcode(&self) -> bool {
        self.fixed() == 0b10000
    }

    pub fn is_adrp(&self) -> bool {
        self.is_adrp_raw() == 1
    }

    pub fn target_pc(&self, pc: u64) -> u64 {
        let unsigned_imm = self.imm_hi() << 2 | self.imm_lo();
        let signed_imm = ((unsigned_imm << 12) as i32 >> 12) as i64;

        if self.is_adrp() {
            (pc & !0xFFF).wrapping_add_signed(signed_imm * 0x1000)
        }
        else {
            pc.wrapping_add_signed(signed_imm)
        }
    }
}

// https://developer.arm.com/documentation/ddi0602/2022-09/Base-Instructions/B--Branch-
bitflags! {
    pub struct Branch: u32 {
        #[signed(i32)]
        pub imm set_imm try_set_imm: 0..26,
        fixed: 26..32,
    }
}

impl Branch {
    pub fn try_from_raw(raw: u32) -> Result<Self, ()> {
        let ins = Self::from_raw(raw);
        ins.assert_opcode().then_some(ins).ok_or(())
    }

    pub fn assert_opcode(&self) -> bool {
        self.fixed() == 0b101
    }

    pub fn target_pc(&self, pc: u64) -> u64 {
        pc.wrapping_add_signed(self.imm() as i64 * 4)
    }

    pub fn try_set_target_pc(&mut self, pc: u64, target: u64) -> Result<(), ()> {
        let diff = target as i64 - pc as i64;
        if diff % 4 != 0 {
            return Err(());
        }
        let imm32 = (diff / 4).try_into().map_err(|_| ())?;
        self.try_set_imm(imm32)
    }
}

// https://developer.arm.com/documentation/ddi0602/2022-09/Base-Instructions/LDR--immediate---Load-Register--immediate--
bitflags! {
    pub struct LdrOfs: u32 {
        pub reg_dest set_reg_dest try_set_reg_dest: 0..5,
        pub reg_base set_reg_base try_set_reg_base: 5..10,
        disp_raw set_disp_raw try_set_disp_raw: 10..22,
        opc set_opc: 22..24,
        fixed set_fixed: 24..30,
        scale set_scale: 30..32
    }
}

impl LdrOfs {
    pub fn new(opc: LdrImmOpc, reg_dest: u32, reg_base: u32, disp: u32) -> Result<Self, Error> {
        let mut ins = Self::from_raw(0);
        ins.set_fixed(0b111001);
        ins.try_set_reg_dest(reg_dest)?;
        ins.try_set_reg_base(reg_base)?;

        match opc {
            LdrImmOpc::Load32 => {
                ins.set_opc(0b01);
                ins.set_scale(0b10);
            }
            LdrImmOpc::Load64 => {
                ins.set_opc(0b01);
                ins.set_scale(0b11);
            }
            LdrImmOpc::Load32Sx => {
                ins.set_opc(0b10);
                ins.set_scale(0b10);
            }
            LdrImmOpc::Prefetch => {
                ins.set_opc(0b10);
                ins.set_scale(0b11);
            }
        }

        ins.try_set_disp(disp)?;

        Ok(ins)
    }

    pub fn disp(&self) -> u32 {
        self.disp_raw() << self.scale()
    }

    pub fn try_set_disp(&mut self, disp: u32) -> Result<(), ()> {
        let scale = self.scale();

        if !disp.is_multiple_of(1 << scale) {
            return Err(());
        }

        self.try_set_disp_raw(disp >> scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aarch64_encoding_ldr_imm() {
        struct Case {
            opc: LdrImmOpc,
            reg: u32,
            imm: i32,
            expected: u32,
        }
        impl Case {
            pub const fn new(
                opc: LdrImmOpc,
                reg: u32,
                imm: i32,
                expected: &'static [u8; 4],
            ) -> Self {
                Case {
                    opc,
                    reg,
                    imm,
                    expected: u32::from_le_bytes(*expected),
                }
            }
        }

        const CASES: &[Case] = &[
            Case::new(LdrImmOpc::Load64, 5, 4, b"\x85\x00\x00\x58"),
            Case::new(LdrImmOpc::Load64, 3, -4, b"\x83\xff\xff\x58"),
            Case::new(LdrImmOpc::Load32, 3, 5, b"\xa3\x00\x00\x18"),
            Case::new(LdrImmOpc::Load32Sx, 3, 5, b"\xa3\x00\x00\x98"),
            Case::new(LdrImmOpc::Prefetch, 0, 10, b"\x40\x01\x00\xd8"),
        ];

        for case in CASES {
            let ldr = LdrImm::from_raw(case.expected);

            assert!(ldr.assert_opcode());
            assert_eq!(ldr.opc(), case.opc);
            assert_eq!(ldr.reg(), case.reg);
            assert_eq!(ldr.imm(), case.imm);

            let encoded = LdrImm::new(case.opc, case.reg, case.imm).expect("encoding failure");
            assert_eq!(encoded.to_raw(), case.expected)
        }
    }

    #[test]
    fn aarch64_encoding_adr() {
        struct Case {
            is_adrp: bool,
            reg: u32,
            pc: u64,
            target: u64,
            expected: u32,
        }
        impl Case {
            pub const fn new(
                is_adrp: bool,
                reg: u32,
                pc: u64,
                target: u64,
                expected: &'static [u8; 4],
            ) -> Self {
                Case {
                    is_adrp,
                    reg,
                    pc,
                    target,
                    expected: u32::from_le_bytes(*expected),
                }
            }
        }

        const CASES: &[Case] = &[
            Case::new(false, 3, 0x1000, 0x2000, b"\x03\x80\x00\x10"),
            Case::new(false, 4, 0x2000, 0x1000, b"\x04\x80\xff\x10"),
            Case::new(true, 5, 0x2000, 0x5000, b"\x05\x00\x00\xf0"),
            Case::new(true, 5, 0x2300, 0x5000, b"\x05\x00\x00\xf0"),
            Case::new(true, 5, 0x5100, 0x2000, b"\xe5\xff\xff\xb0"),
        ];

        for case in CASES {
            let adr = Adr::from_raw(case.expected);

            assert!(adr.assert_opcode());
            assert_eq!(adr.is_adrp(), case.is_adrp);
            assert_eq!(adr.reg(), case.reg);
            assert_eq!(adr.target_pc(case.pc), case.target);
        }
    }

    #[test]
    fn aarch64_encoding_branch() {
        struct Case {
            pc: u64,
            target: u64,
            expected: u32,
        }
        impl Case {
            pub const fn new(pc: u64, target: u64, expected: &'static [u8; 4]) -> Self {
                Case {
                    pc,
                    target,
                    expected: u32::from_le_bytes(*expected),
                }
            }
        }

        const CASES: &[Case] = &[
            Case::new(0x1000, 0x2000, b"\x00\x04\x00\x14"),
            Case::new(0x2000, 0x1000, b"\x00\xfc\xff\x17"),
        ];

        for case in CASES {
            let b = Branch::from_raw(case.expected);

            assert!(b.assert_opcode());
            assert_eq!(b.target_pc(case.pc), case.target);
        }
    }

    #[test]
    fn aarch64_encoding_ldr_ofs() {
        struct Case {
            opc: LdrImmOpc,
            reg_dest: u32,
            reg_base: u32,
            disp: u32,
            expected: u32,
        }
        impl Case {
            pub const fn new(
                opc: LdrImmOpc,
                reg_dest: u32,
                reg_base: u32,
                disp: u32,
                expected: &'static [u8; 4],
            ) -> Self {
                Case {
                    opc,
                    reg_dest,
                    reg_base,
                    disp,
                    expected: u32::from_le_bytes(*expected),
                }
            }
        }

        const CASES: &[Case] = &[
            Case::new(LdrImmOpc::Load64, 3, 4, 0x100, b"\x83\x80\x40\xf9"),
            Case::new(LdrImmOpc::Load32, 3, 4, 0x100, b"\x83\x00\x41\xb9"),
            Case::new(LdrImmOpc::Load32Sx, 3, 4, 0x100, b"\x83\x00\x81\xb9"),
            Case::new(LdrImmOpc::Prefetch, 0, 3, 0x200, b"\x60\x00\x81\xf9"),
        ];

        for case in CASES {
            let ldr = LdrOfs::from_raw(case.expected);

            assert_eq!(ldr.reg_dest(), case.reg_dest);
            assert_eq!(ldr.reg_base(), case.reg_base);
            assert_eq!(ldr.disp(), case.disp);

            let encoded = LdrOfs::new(case.opc, case.reg_dest, case.reg_base, case.disp)
                .expect("encoding failure");
            assert_eq!(encoded.to_raw(), case.expected)
        }
    }
}
