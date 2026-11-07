#![cfg(feature = "default_jit_alloc")]

use core::{
    alloc::Layout,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

use alloc::collections::BTreeMap;
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
    rw_to_rx: BTreeMap<*mut u8, *const u8>,
    rx_to_rw: BTreeMap<*const u8, *mut u8>,
}

struct JitWriteProtect;

impl JitTlsf {
    pub const fn new() -> Self {
        Self {
            inner: FlexTlsf::new(Source::new()),
        }
    }

    pub fn alloc(&mut self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        let layout = Layout::from_size_align(size, 1).map_err(|_| JitAllocError)?;

        let rw = {
            let _write_protect = JitWriteProtect::new();
            self.inner.allocate(layout).ok_or(JitAllocError)?.as_ptr()
        };

        let (rw_end, rx_end) =
            self.inner.source_ref().rw_to_rx.range(rw..).next().ok_or(JitAllocError)?;

        let offset = unsafe { rw_end.offset_from_unsigned(rw) };
        let rx = unsafe { rx_end.sub(offset) };

        Ok((rx, rw))
    }

    pub unsafe fn release(&mut self, rx: *const u8) -> Result<(), JitAllocError> {
        let (rx_end, rw_end) =
            self.inner.source_ref().rx_to_rw.range(rx..).next().ok_or(JitAllocError)?;

        let offset = unsafe { rx_end.offset_from_unsigned(rx) };
        let rw = unsafe { rw_end.sub(offset) };

        let rw = NonNull::new(rw).ok_or(JitAllocError)?;

        let _write_protect = JitWriteProtect::new();
        self.inner.deallocate(rw, 1);

        Ok(())
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
        libc::pthread_jit_write_protect_np(0);
        Self
    }
}

impl Drop for JitWriteProtect {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        libc::pthread_jit_write_protect_np(1);
    }
}
