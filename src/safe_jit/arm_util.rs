use alloc::{borrow::Cow, vec::Vec};

use capstone::{InsnGroupId, InsnGroupType};

#[cfg_attr(target_arch = "aarch64", path = "arm_util/encoding_aarch64.rs")]
#[cfg_attr(
    all(target_arch = "arm", not(thumb_mode)),
    path = "arm_util/encoding_arm.rs"
)]
#[cfg_attr(
    all(target_arch = "arm", thumb_mode),
    path = "arm_util/encoding_thumb.rs"
)]
pub mod encoding;

/// Check if any of the instruction groups provided is unsupported by the arm/aarch64 relocator.
pub fn has_unsupported_insn_group(groups: &[InsnGroupId]) -> bool {
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

    groups
        .iter()
        .any(|g| UNSUPPORTED_GROUPS.contains(&(g.0 as InsnGroupType::Type)))
}

#[derive(Debug, Clone, Copy)]
pub struct EncodingError;

impl From<()> for EncodingError {
    fn from(_value: ()) -> Self {
        EncodingError
    }
}

impl From<EncodingError> for JitError {
    fn from(_value: EncodingError) -> Self {
        JitError::EncodingError
    }
}

/// Copy-on-write growable buffer with the ability to replace bytes at a particular offset and
/// append bytes at increasing offsets in the buffer with minimal (re)allocations.
pub struct CowBuffer<'a> {
    orig_bytes: &'a [u8],
    new_bytes: Vec<u8>,
    num_copied: usize,
}

impl<'a> CowBuffer<'a> {
    pub fn new(orig_bytes: &'a [u8]) -> Self {
        Self {
            orig_bytes,
            new_bytes: Vec::new(),
            num_copied: 0,
        }
    }

    pub fn into_bytes(mut self) -> Cow<'a, [u8]> {
        if self.new_bytes.is_empty() {
            self.orig_bytes.into()
        }
        else {
            // write the remaining slice
            self.new_bytes.extend_from_slice(&self.orig_bytes[self.num_copied..]);
            self.new_bytes.into()
        }
    }

    #[allow(unused)]
    /// Get a reference to the new buffer.
    pub fn new_bytes(&self) -> &[u8] {
        &self.new_bytes
    }

    /// Get a mutable reference to the new buffer.
    ///
    /// You may grow this buffer arbitrarily, but shrinking it will lead to logic errors.
    pub fn new_bytes_mut(&mut self) -> &mut Vec<u8> {
        &mut self.new_bytes
    }

    /// Copy bytes from the original slice into [`Self::new_bytes`] up to `offset` in the original
    /// slice.
    pub fn copy_up_to(&mut self, offset: usize) {
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
    pub fn append(&mut self, offset: usize, bytes: &[u8]) -> usize {
        if !bytes.is_empty() {
            self.copy_up_to(offset);
        }
        let bytes_offset = self.new_bytes.len();
        self.new_bytes.extend_from_slice(bytes);
        bytes_offset
    }

    /// Ignore `count` bytes at `offset` in the original slice.
    ///
    /// Returns the offset at which said bytes would have been in [`Self::new_bytes`].
    #[allow(unused)]
    pub fn ignore(&mut self, offset: usize, count: usize) -> usize {
        debug_assert!(offset + count <= self.orig_bytes.len());

        self.copy_up_to(offset);
        self.num_copied += count;
        self.new_bytes.len()
    }

    /// Replace the bytes at `offset` in the original slice.
    ///
    /// Returns the offset of `bytes` in [`Self::new_bytes`].
    #[allow(unused)]
    pub fn replace(&mut self, offset: usize, bytes: &[u8]) -> usize {
        debug_assert!(offset + bytes.len() <= self.orig_bytes.len());

        let bytes_offset = self.append(offset, bytes);
        self.num_copied += bytes.len();
        bytes_offset
    }
}

/// basic bitflags macro for use on arm/aarch64 where we need to diy our own encoders.
macro_rules! bitflags {
    ($($(#[$attrs:meta])* $v:vis struct $name:ident: $t:ty {
        $(
            $(#[signed($signed_ty:ty)])?
            $fvis:vis
            $getter:ident $($setter:ident $($try_setter:ident)?)? :
            $start:literal .. $end:literal
        ),*$(,)?
    })+) => { $(
        $(#[$attrs])*
        #[derive(Clone, Copy)]
        #[repr(transparent)]
        $v struct $name($t);

        #[allow(unused)]
        impl $name {
            pub const fn from_raw(val: $t) -> Self {
                Self(val)
            }

            pub const fn to_raw(self) -> $t {
                self.0
            }

            $(
                $crate::safe_jit::arm_util::bitflags_field! {
                    ($t, $($signed_ty)?) $fvis $getter $($setter $($try_setter)?)?: $start .. $end
                }
            )*
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($name))
                    $(.field(stringify!($getter), &crate::safe_jit::arm_util::FmtDecBin(self.$getter())))*
                    .finish()
            }
        }
    )+ };
}

macro_rules! bitflags_field {
    (   ($t:ty,)
        $fvis:vis
        $getter:ident $($setter:ident $($try_setter:ident)?)? :
        $start:literal .. $end:literal
    ) => {
        $fvis fn $getter(&self) -> $t {
            const SIZE: u32 = $end - $start;
            const SIZE_MASK: $t = ((1u128 << SIZE) - 1) as $t;
            (self.0 >> $start) & SIZE_MASK
        }

        $(
            $fvis fn $setter(&mut self, val: $t) {
                const SIZE: u32 = $end - $start;
                const SIZE_MASK: $t = ((1u128 << SIZE) - 1) as $t;
                const MASK: $t = SIZE_MASK << ($start as u32);

                self.0 = (self.0 & !MASK) | ((val & SIZE_MASK) << $start);
            }

            $($fvis fn $try_setter(&mut self, val: $t) -> Result<(), ()> {
                const SIZE: u32 = $end - $start;
                const SIZE_MASK: $t = ((1u128 << SIZE) - 1) as $t;
                const MASK: $t = SIZE_MASK << ($start as u32);

                if val > SIZE_MASK {
                    return Err(())
                }

                self.0 = (self.0 & !MASK) | ((val & SIZE_MASK) << $start);
                Ok(())
            })?
        )?
    };

    (   ($t:ty, $signed_ty:ty)
        $fvis:vis
        $getter:ident $($setter:ident $($try_setter:ident)?)? :
        $start:literal .. $end:literal
    ) => {
        $fvis fn $getter(&self) -> $signed_ty {
            const BITS: u32 = 8 * ::core::mem::size_of::<$signed_ty>() as u32;
            const LSHIFT: u32 = BITS - $end;
            const RSHIFT: u32 = LSHIFT + $start;

            (self.0 as $signed_ty) << LSHIFT >> RSHIFT
        }

        $(
            $fvis fn $setter(&mut self, val: $signed_ty) {
                const SIZE: u32 = $end - $start;
                const SIZE_MASK: $t = ((1u128 << SIZE) - 1) as $t;
                const MASK: $t = SIZE_MASK << ($start as u32);

                self.0 = (self.0 & !MASK) | ((val as $t & SIZE_MASK)  << $start);
            }

            $($fvis fn $try_setter(&mut self, val: $signed_ty) -> Result<(), ()> {
                const SIZE: u32 = $end - $start;
                const SIZE_MASK: $t = ((1u128 << SIZE) - 1) as $t;
                const MASK: $t = SIZE_MASK << ($start as u32);

                let extra_bits = val >> SIZE;
                if extra_bits != -1 && extra_bits != 0 {
                    return Err(())
                }

                self.0 = (self.0 & !MASK) | ((val as $t & SIZE_MASK) << $start);
                Ok(())
            })?
        )?
    };
}

pub(crate) use bitflags;
pub(crate) use bitflags_field;

use crate::safe_jit::JitError;

// for the bitflags debug impl
#[allow(unused)]
pub(crate) struct FmtDecBin<T>(pub T);
impl<T: ::core::fmt::Display + ::core::fmt::Binary> ::core::fmt::Debug for FmtDecBin<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{0} (0b{0:b})", self.0))
    }
}
