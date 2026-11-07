#![cfg(all(unix, feature = "default_jit_alloc"))]
#![allow(clippy::unnecessary_cast, clippy::useless_conversion)]

use core::{
    ffi::{c_char, c_int, CStr},
    hash::{Hash, Hasher},
    hint,
    ops::ControlFlow,
    ptr,
    sync::atomic::{AtomicU32, Ordering::Relaxed},
};

use alloc::{borrow::ToOwned, format};
use libc::{
    clock_gettime, close, dup, fclose, fileno, ftruncate, getenv, getpid, gettid, mmap, munmap,
    off_t, open, size_t, sysconf, tmpfile, _SC_PAGESIZE, CLOCK_MONOTONIC, CLOCK_REALTIME,
    MAP_FAILED, MAP_SHARED, O_CREAT, O_EXCL, O_RDWR, PROT_EXEC, PROT_READ, PROT_WRITE, S_IRUSR,
    S_IWUSR,
};

#[allow(unused)]
const PROT_RX: c_int = PROT_READ | PROT_EXEC;
#[allow(unused)]
const PROT_RW: c_int = PROT_READ | PROT_WRITE;
#[allow(unused)]
const PROT_RWX: c_int = PROT_READ | PROT_WRITE | PROT_EXEC;

/// Returns the minimum virtual memory allocation granularity.
pub fn map_granularity() -> usize {
    let granularity = unsafe { sysconf(_SC_PAGESIZE) as usize };
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
    debug_assert!(size.is_multiple_of(map_granularity()));
    BackingFd::with_global(|fd| unsafe { fd.mmap_rx_rw(size, 0) })?
}

#[derive(Debug)]
struct BackingFd {
    fd: c_int,
    offset: usize,
}

impl BackingFd {
    fn with_global<T>(action: impl FnOnce(&mut Self) -> T) -> Option<T> {
        let mut maybe_fd = cfg_select! {
            feature = "std" => {{
                static FD: std::sync::Mutex<Option<Option<BackingFd>>> =
                    std::sync::Mutex::new(None);
                FD.lock().unwrap()
            }}
            _ => {{
                static FD: spin::Mutex<Option<Option<BackingFd>>> = spin::Mutex::new(None);
                FD.lock()
            }}
        };

        let fd = maybe_fd.get_or_insert_with(Self::open).as_mut()?;
        Some(action(fd))
    }

    fn open() -> Option<Self> {
        fn try_open(page_size: size_t, fd_src: impl FnOnce() -> c_int) -> ControlFlow<BackingFd> {
            unsafe {
                let fd = fd_src();

                if fd == -1 {
                    return ControlFlow::Continue(());
                }

                if ftruncate(fd, page_size as off_t) == -1 {
                    close(fd);
                    return ControlFlow::Continue(());
                }

                let rx = mmap(ptr::null_mut(), page_size, PROT_RX, MAP_SHARED, fd, 0);

                if rx == MAP_FAILED {
                    close(fd);
                    return ControlFlow::Continue(());
                }

                munmap(rx, page_size);

                ControlFlow::Break(BackingFd { fd, offset: 0 })
            }
        }

        fn do_open(page_size: usize) -> ControlFlow<BackingFd> {
            try_open(page_size, open_linux)?;
            try_open(page_size, open_freebsd)?;
            try_open(page_size, open_named_shm)?;
            try_open(page_size, open_tmpfile)?;
            try_open(page_size, open_in_tmpdir)
        }

        let page_size = unsafe { sysconf(_SC_PAGESIZE) as size_t };
        do_open(page_size).break_value()
    }

    unsafe fn mmap_rx_rw(&mut self, len: usize, flags: c_int) -> Option<(*const u8, *mut u8)> {
        let offset = self.offset;
        let new_len = self.offset.checked_add(len)?;

        if unsafe { ftruncate(self.fd, new_len.try_into().ok()?) != 0 } {
            return None;
        };

        let len = len.try_into().ok()?;
        let offset = offset.try_into().ok()?;

        self.offset = new_len;

        cfg_select! {
            target_os = "macos" => {
                let flags = flags | libc::MAP_PRIVATE | libc::MAP_JIT;

                let rwx = unsafe { mmap(ptr::null_mut(), len, PROT_RWX, flags, self.fd, offset) };

                if rwx == MAP_FAILED {
                    return None;
                }

                Some((rwx as *const u8, rwx as *mut u8))
            }
            _ => {
                let flags = flags | libc::MAP_SHARED;

                let rx = unsafe { mmap(ptr::null_mut(), len, PROT_RX, flags, self.fd, offset) };

                if rx == MAP_FAILED {
                    return None;
                }

                let rw = unsafe { mmap(ptr::null_mut(), len, PROT_RW, flags, self.fd, offset) };

                if rw == MAP_FAILED {
                    munmap(rx, len);
                    return None;
                }

                Some((rx as *const u8, rw as *mut u8))
            }
        }
    }
}

fn open_linux() -> c_int {
    cfg_select! {
        target_os = "linux" => unsafe {
            libc::memfd_create(c"closure-ffi".as_ptr(), libc::MFD_CLOEXEC)
        }
        _ => -1
    }
}

fn open_freebsd() -> c_int {
    cfg_select! {
        target_os = "freebsd" => unsafe {
            libc::shm_open(libc::SHM_ANON, O_RDWR | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR)
        }
        _ => -1
    }
}

fn open_named_shm() -> c_int {
    cfg_select! {
        not(any(target_vendor = "apple", target_os = "android")) => unsafe {
            let id = random_id();
            let name = alloc::format!("/closure-ffi-{id:016x}\0");

            libc::shm_open(name.as_ptr() as *const c_char, O_RDWR | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR)
        }
        _ => -1
    }
}

fn open_tmpfile() -> c_int {
    unsafe {
        let file = tmpfile();

        if file.is_null() {
            return -1;
        }

        let mut fd = fileno(file);
        if fd != -1 {
            fd = dup(fd);
        }

        fclose(file);

        fd
    }
}

fn open_in_tmpdir() -> c_int {
    unsafe {
        let tmp = getenv(c"TMPDIR".as_ptr());

        let mut tmp_dir = if !tmp.is_null() {
            CStr::from_ptr(tmp).to_owned().into_bytes()
        } else {
            b"/tmp".to_vec()
        };

        let id = random_id();
        tmp_dir.extend_from_slice(format!("/closure-ffi-{id:016x}\0").as_bytes());

        let flags = cfg_select! {
            target_os = "linux" => O_RDWR | O_CREAT | O_EXCL | libc::O_CLOEXEC,
            _ => O_RDWR | O_CREAT | O_EXCL,
        };

        open(tmp_dir.as_ptr() as *const c_char, flags, S_IRUSR | S_IWUSR)
    }
}

fn random_id() -> u64 {
    static STATE: AtomicU32 = AtomicU32::new(0xdeadbeef);

    // TODO: replace with DefaultHasher when it's added to core.
    #[allow(deprecated)]
    let mut hasher = core::hash::SipHasher::new();

    STATE.fetch_add(1, Relaxed).hash(&mut hasher);

    hint::black_box(&raw mut hasher).hash(&mut hasher);

    unsafe {
        let mut ts_real = core::mem::zeroed();
        if clock_gettime(CLOCK_REALTIME, &mut ts_real) != -1 {
            ts_real.tv_nsec.hash(&mut hasher);
            ts_real.tv_sec.hash(&mut hasher);
        }

        let mut ts_monotonic = core::mem::zeroed();
        if clock_gettime(CLOCK_MONOTONIC, &mut ts_monotonic) != -1 {
            ts_monotonic.tv_nsec.hash(&mut hasher);
            ts_monotonic.tv_sec.hash(&mut hasher);
        }

        gettid().hash(&mut hasher);

        getpid().hash(&mut hasher);
    }

    let hash = hasher.finish();

    STATE.fetch_add(hash as u32, Relaxed);

    hash
}
