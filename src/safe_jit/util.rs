// for the bitflags debug impl
pub(crate) struct FmtAll<T>(pub T);
impl<T: ::core::fmt::Display + ::core::fmt::LowerHex + ::core::fmt::Binary> ::core::fmt::Debug
    for FmtAll<T>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{0} / 0x{0:x} / 0b{0:b}", self.0))
    }
}

/// basic bitflags macro for use on arm/aarch64 where we need to diy our own encoders.
macro_rules! bitflags {
    ($v:vis struct $name:ident: $t:ty {
        $(
            $(#[signed($signed_ty:ty)])?
            $fvis:vis
            $getter:ident $($setter:ident $($try_setter:ident)?)? :
            $start:literal .. $end:literal
        ),*$(,)?
    }) => {
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
                $crate::safe_jit::util::bitflags_field! {
                    ($t, $($signed_ty)?) $fvis $getter $($setter $($try_setter)?)?: $start .. $end
                }
            )*
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($name))
                    $(.field(stringify!($getter), &crate::safe_jit::util::FmtAll(self.$getter())))*
                    .finish()
            }
        }
    };
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
#[allow(unused)]
pub(crate) use bitflags_field;
