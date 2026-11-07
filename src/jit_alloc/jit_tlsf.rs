#![cfg(feature = "default_jit_alloc")]

use alloc::collections::BTreeMap;
use core::{
    alloc::Layout,
    ops::Bound::{Excluded, Unbounded},
    ptr::NonNull,
    slice,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

use rlsf::{FlexSource, FlexTlsf};

#[cfg(unix)]
use crate::jit_alloc::unix::{map_granularity, map_rx_rw};
#[cfg(windows)]
use crate::jit_alloc::windows::{map_granularity, map_rx_rw};
use crate::JitAllocError;

pub struct JitTlsf {
    inner: FlexTlsf<Source, u32, u16, { u32::BITS as usize }, { u16::BITS as usize }>,
}

#[derive(Debug)]
struct Source {
    granularity: AtomicUsize,

    /// Maps past-the-end pointers of rw blocks to past-the-end pointers of rx blocks.
    rw_to_rx: BTreeMap<*mut u8, *const u8>,

    /// Maps past-the-end pointers of rx blocks to past-the-end pointers of rw blocks.
    rx_to_rw: BTreeMap<*const u8, *mut u8>,
}

/// Using the allocator on macOS requires this guard which calls `pthread_jit_write_protect_np`
/// on [`Self::new`] and on [`Self::drop`].
struct JitWriteProtect;

impl JitTlsf {
    pub const fn new() -> Self {
        Self {
            inner: FlexTlsf::new(Source::new()),
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> Result<(*const u8, *mut u8), JitAllocError> {
        let layout = Self::pad_layout(layout)?;

        // Allocate, write protect and fill memory with trap instructions.
        let rw = unsafe {
            let _write_protect = JitWriteProtect::new();

            let rw = self.inner.allocate(layout).ok_or(JitAllocError)?.as_ptr();

            let bytes = slice::from_raw_parts_mut(rw, layout.size());
            bytes.align_to_mut::<TrapCode>().1.fill(TRAP);

            rw
        };

        // Matching an in-block pointer against past-the-end we don't want to include
        // the end of a previous block by accident.
        let (rw_end, rx_end) = self
            .inner
            .source_ref()
            .rw_to_rx
            .range((Excluded(&rw), Unbounded))
            .next()
            .ok_or(JitAllocError)?;

        // Calculate offset in rx from the end.
        let offset = unsafe { rw_end.offset_from_unsigned(rw) };
        let rx = unsafe { rx_end.sub(offset) };

        // Overwrote entire range with trap instructions, so flush.
        unsafe {
            clear_cache::clear_cache(rx, rx.add(layout.size()));
        }

        Ok((rx, rw))
    }

    pub unsafe fn release(&mut self, rx: *const u8, layout: Layout) -> Result<(), JitAllocError> {
        let layout = Self::pad_layout(layout)?;

        // Matching an in-block pointer against past-the-end we don't want to include
        // the end of a previous block by accident.
        let (rx_end, rw_end) = self
            .inner
            .source_ref()
            .rx_to_rw
            .range((Excluded(&rx), Unbounded))
            .next()
            .ok_or(JitAllocError)?;

        // Calculate offset in rw from the end.
        let offset = unsafe { rx_end.offset_from_unsigned(rx) };
        let rw = unsafe { rw_end.sub(offset) };

        let rw = NonNull::new(rw).ok_or(JitAllocError)?;

        // Write protect and free memory, fill with trap instructions.
        unsafe {
            let _write_protect = JitWriteProtect::new();

            let bytes = slice::from_raw_parts_mut(rw.as_ptr(), layout.size());
            bytes.align_to_mut::<TrapCode>().1.fill(TRAP);

            clear_cache::clear_cache(rx, rx.add(size_of::<TrapCode>()));

            self.inner.deallocate(rw, layout.align());
        }

        Ok(())
    }

    /// Always allocates enough memory to append at least one trap instruction.
    ///
    /// [`Self::release`] relies on this fact to write at least one trap instruction at the start.
    fn pad_layout(layout: Layout) -> Result<Layout, JitAllocError> {
        layout
            .extend(Layout::new::<TrapCode>())
            .map(|(layout, _)| layout.pad_to_align())
            .map_err(|_| JitAllocError)
    }
}

impl Source {
    /// The source will return allocations at least this big.
    const MIN_SIZE: usize = 0x10000;

    const fn new() -> Self {
        Self {
            granularity: AtomicUsize::new(0),
            rw_to_rx: BTreeMap::new(),
            rx_to_rw: BTreeMap::new(),
        }
    }

    fn granularity(&self) -> usize {
        let mut granularity = 0;

        let _ = self.granularity.try_update(Relaxed, Relaxed, |maybe_granularity| {
            match maybe_granularity {
                0 => {
                    granularity = map_granularity();
                    Some(granularity)
                }
                _ => {
                    granularity = maybe_granularity;
                    None
                }
            }
        });

        granularity
    }
}

unsafe impl FlexSource for Source {
    unsafe fn alloc(&mut self, min_size: usize) -> Option<NonNull<[u8]>> {
        let size = min_size.max(Self::MIN_SIZE).checked_next_multiple_of(self.granularity())?;

        let (rx, rw) = unsafe { map_rx_rw(size)? };

        let rx_end = rx.wrapping_add(size);
        let rw_end = rw.wrapping_add(size);

        self.rx_to_rw.insert(rx_end, rw_end);
        self.rw_to_rx.insert(rw_end, rx_end);

        Some(NonNull::slice_from_raw_parts(
            NonNull::new(rw).expect("map_rx_rw returned null?"),
            size,
        ))
    }

    fn min_align(&self) -> usize {
        self.granularity()
    }
}

unsafe impl Send for Source {}

unsafe impl Sync for Source {}

impl JitWriteProtect {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        unsafe {
            libc::pthread_jit_write_protect_np(0);
        }
        Self
    }
}

impl Drop for JitWriteProtect {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        unsafe {
            libc::pthread_jit_write_protect_np(1);
        }
    }
}

cfg_select! {
    any(target_arch = "x86", target_arch = "x86_64") => {
        /// Trap instruction: int3
        const TRAP: TrapCode = 0xcc;
        type TrapCode = u8;
    }
    target_arch = "aarch64" => {
        /// Trap instruction: brk #0xF000
        const TRAP: TrapCode = TrapCode::from_be_bytes([0xd4, 0x3e, 0x00, 0x00]);
        type TrapCode = u32;
    }
    all(target_arch = "arm", thumb_mode) => {
        /// Trap instruction: bkpt #0
        const TRAP: TrapCode = TrapCode::from_be_bytes([0xbe, 0x00]);
        type TrapCode = u16;
    }
    target_arch = "arm" => {
        /// Trap instruction: bkpt #0
        const TRAP: TrapCode = TrapCode::from_be_bytes([0xe1, 0x20, 0x00, 0x70]);
        type TrapCode = u32;
    }
    _ => {
        /// Trap instruction zerofill fallback
        const TRAP: TrapCode = 0;
        type TrapCode = u32;
    }
}

#[cfg(test)]
mod tests {
    use core::alloc::Layout;

    use crate::jit_alloc::jit_tlsf::{JitTlsf, TrapCode, TRAP};

    static ALLOC: spin::Mutex<JitTlsf> = spin::Mutex::new(JitTlsf::new());

    #[test]
    fn new_allocations_trap() {
        let layout = Layout::from_size_align(1, 1).unwrap();
        let (rx, _) = ALLOC.lock().alloc(layout).unwrap();

        let trap = unsafe { rx.cast::<TrapCode>().read() };
        assert_eq!(trap, TRAP);
    }
}
