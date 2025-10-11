use alloc::borrow::Cow;

use super::RelocError;

pub fn try_reloc_thunk_template<'a>(
    thunk_template: &'a [u8],
    ip: u64,
    magic_offset: usize,
) -> Result<Cow<'a, [u8]>, RelocError> {
    Ok(Cow::Borrowed(thunk_template))
}
