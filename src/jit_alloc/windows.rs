#![cfg(all(windows, feature = "default_jit_alloc"))]

use core::{mem, ptr};

use crate::jit_alloc::windows::ffi::{
    CloseHandle, CreateFileMappingW, GetSystemInfo, MapViewOfFile, UnmapViewOfFile, DWORD,
    FILE_MAP_EXECUTE, FILE_MAP_READ, FILE_MAP_WRITE, INVALID_HANDLE_VALUE, PAGE_EXECUTE_READWRITE,
};

mod ffi;

/// Returns the minimum virtual memory allocation granularity.
pub fn map_granularity() -> usize {
    let sys_info = unsafe {
        let mut info = mem::zeroed();
        GetSystemInfo(&mut info);
        info
    };

    let granularity = sys_info.dwAllocationGranularity as usize;
    debug_assert!(granularity.is_power_of_two());

    granularity
}

/// Allocates a read-execute and a read-write mapping pointing to the same memory.
///
/// The mappings may overlap and `pthread_jit_write_protect_np` must be called before
/// writing to the mapping on macOS.
///
/// # Safety
///
/// `size` must be a multiple of the value returned by [`map_granularity`].
pub unsafe fn map_rx_rw(size: usize) -> Option<(*const u8, *mut u8)> {
    unsafe {
        debug_assert!(size.is_multiple_of(map_granularity()));

        let mapping = CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            ptr::null_mut(),
            PAGE_EXECUTE_READWRITE,
            (size >> 32) as DWORD,
            size as DWORD,
            ptr::null_mut(),
        );

        if mapping.is_null() {
            return None;
        }

        let rx = MapViewOfFile(mapping, FILE_MAP_READ | FILE_MAP_EXECUTE, 0, 0, 0);

        if rx.is_null() {
            let _ = CloseHandle(mapping);
            return None;
        }

        let rw = MapViewOfFile(mapping, FILE_MAP_READ | FILE_MAP_WRITE, 0, 0, 0);

        // The mapping handle is closed regardless.
        let _ = CloseHandle(mapping);

        if rw.is_null() {
            let _ = UnmapViewOfFile(rx);
            return None;
        }

        Some((rx as *const u8, rw as *mut u8))
    }
}
