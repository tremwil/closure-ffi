#![cfg(all(unix, feature = "default_jit_alloc"))]
#![allow(clippy::unnecessary_cast, clippy::useless_conversion)]

use alloc::{borrow::ToOwned, format};
use core::{
    ffi::{c_char, c_int, c_uint, CStr},
    hash::{Hash, Hasher},
    hint,
    ops::ControlFlow,
    ptr,
    sync::atomic::{AtomicU32, Ordering::Relaxed},
};

use libc::{
    clock_gettime, close, dup, fclose, fileno, ftruncate, getenv, getpid, mmap, munmap, off_t,
    open, size_t, sysconf, tmpfile, unlink, _SC_PAGESIZE, CLOCK_MONOTONIC, CLOCK_REALTIME,
    MAP_ANONYMOUS, MAP_FAILED, MAP_PRIVATE, MAP_SHARED, O_CREAT, O_EXCL, O_RDWR, PROT_EXEC,
    PROT_READ, PROT_WRITE, S_IRUSR, S_IWUSR,
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

    // Use a single backing fd for all maps by calling `ftruncate` and `mmap`-ing
    // at an offset.
    BackingFd::with_global(|fd| unsafe { fd.mmap_rx_rw(size, 0) })?
}

#[derive(Debug)]
struct BackingFd {
    fd: c_int,
    size: usize,
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

    /// Opens a fd that can be mapped as executable.
    fn open() -> Option<Self> {
        /// Tries mapping a fd as executable memory. A security policy might disallow this
        /// for example when /dev/shm or /tmp are mounted as noexec.
        fn try_map(page_size: size_t, fd: c_int) -> ControlFlow<BackingFd> {
            unsafe {
                let (prot, flags) = if fd == -1 {
                    (PROT_RWX, MAP_PRIVATE | MAP_ANONYMOUS)
                }
                else if cfg!(target_os = "macos") {
                    (PROT_RWX, MAP_PRIVATE)
                }
                else {
                    (PROT_RX, MAP_SHARED)
                };

                #[cfg(target_os = "macos")]
                let flags = flags | libc::MAP_JIT;

                let rx = mmap(ptr::null_mut(), page_size, prot, flags, fd, 0);

                if rx == MAP_FAILED {
                    return ControlFlow::Continue(());
                }

                munmap(rx, page_size);

                ControlFlow::Break(BackingFd { fd, size: 0 })
            }
        }

        fn try_map_resize(page_size: size_t, fd: c_int) -> ControlFlow<BackingFd> {
            unsafe {
                if fd != -1 {
                    if ftruncate(fd, page_size as off_t) != -1 {
                        try_map(page_size, fd)?;
                    }

                    close(fd);
                }

                ControlFlow::Continue(())
            }
        }

        /// Attempts multiple strategies for opening a fd that can be mapped as executable.
        ///
        /// The first one to succeed is used:
        ///
        /// 1. `memfd_create` (Linux only)
        /// 2. `shm_open` with `SHM_ANON` (FreeBSD only)
        /// 3. anonymous RWX mapping
        ///
        /// Target feature "shm_open_jit_alloc" only:
        /// 4. `shm_open` with a randomly assigned name
        ///
        /// Target feature "tmp_file_jit_alloc" only:
        /// 5. `tmpfile` into `fileno`
        /// 6. `open` with `O_TMPFILE` in /tmp (Linux only)
        /// 7. `open` with `O_TMPFILE` in the CWD (Linux only)
        /// 8. `open` in /tmp with a randomly assigned name
        fn do_open(page_size: usize) -> ControlFlow<BackingFd> {
            try_map_resize(page_size, open_linux())?;
            try_map_resize(page_size, open_freebsd())?;
            try_map(page_size, -1)?;

            #[cfg(feature = "shm_open_jit_alloc")]
            try_map_resize(page_size, open_named_shm())?;

            #[cfg(feature = "tmp_file_jit_alloc")]
            {
                try_map_resize(page_size, open_tmpfile())?;
                try_map_resize(page_size, open_tmp_in_cwd())?;
                try_map_resize(page_size, open_tmp_in_tmpdir())?;
                try_map_resize(page_size, open_named_in_tmpdir())?;
            }

            ControlFlow::Continue(())
        }

        let page_size = unsafe { sysconf(_SC_PAGESIZE) as size_t };
        do_open(page_size).break_value()
    }

    /// Allocates a read-execute and a read-write mapping pointing to the same memory.
    ///
    /// The mappings may overlap and `pthread_jit_write_protect_np` must be called before
    /// writing to the mapping on macOS.
    ///
    /// # Safety
    ///
    /// `size` must be a multiple of the value returned by [`map_granularity`].
    unsafe fn mmap_rx_rw(&mut self, size: usize, flags: c_int) -> Option<(*const u8, *mut u8)> {
        /// Allocates a single RWX mapping (fallback or macOS specific).
        unsafe fn mmap_rwx(
            len: size_t,
            mut flags: c_int,
            fd: c_int,
            offset: off_t,
        ) -> Option<(*const u8, *mut u8)> {
            flags |= MAP_PRIVATE;

            if fd == -1 {
                flags |= MAP_ANONYMOUS;
            }

            #[cfg(target_os = "macos")]
            {
                flags |= libc::MAP_JIT;
            }

            let rwx = unsafe { mmap(ptr::null_mut(), len, PROT_RWX, flags, fd, offset) };

            if rwx == MAP_FAILED {
                return None;
            }

            Some((rwx as *const u8, rwx as *mut u8))
        }

        let len = size.try_into().ok()?;

        // Didn't acquire a fd for double mapping, fall back to a single anonymous one.
        if self.fd == -1 {
            return mmap_rwx(len, flags, self.fd, 0);
        }

        let offset = self.size;
        let new_size = self.size.checked_add(size)?;

        // Grow the file by `size` to map the newly allocated memory.
        if unsafe { ftruncate(self.fd, new_size.try_into().ok()?) != 0 } {
            return None;
        };

        let offset = offset.try_into().ok()?;

        self.size = new_size;

        cfg_select! {
            // On macOS return a single mapping with the MAP_JIT flag.
            target_os = "macos" => mmap_rwx(len, flags, self.fd, offset),
            // On other platforms return a double mapping.
            _ => {
                let flags = flags | MAP_SHARED;

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
        any(
            target_os = "linux",
            all(target_os = "android", feature = "api-level-30")
        ) => unsafe {
            // N.B. the name used with `memfd_create` does not have to be unique.
            // Use a syscall in case `memfd_create` is not implemented.
            libc::syscall(
                libc::SYS_memfd_create,
                c"closure-ffi".as_ptr(),
                libc::MFD_CLOEXEC,
            ) as c_int
        },
        _ => -1,
    }
}

fn open_freebsd() -> c_int {
    cfg_select! {
        target_os = "freebsd" => unsafe {
            libc::shm_open(libc::SHM_ANON, O_RDWR | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR)
        },
        _ => -1,
    }
}

fn open_named_shm() -> c_int {
    cfg_select! {
        not(any(target_vendor = "apple", target_os = "android")) => unsafe {
            let id = random_id();

            let name = alloc::format!("/closure-ffi-{id:016x}\0");
            let name_ptr = name.as_ptr() as *const c_char;

            let fd = libc::shm_open(
                name.as_ptr() as *const c_char,
                O_RDWR | O_CREAT | O_EXCL,
                S_IRUSR | S_IWUSR,
            );

            if fd != -1 {
                libc::shm_unlink(name_ptr);
            }

            fd
        },
        _ => -1,
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
            // This fd still belongs to the stream, dup it.
            fd = dup(fd);
        }

        // Close the stream (and the original fd).
        fclose(file);

        fd
    }
}

fn open_tmp_in_cwd() -> c_int {
    open_tmp_in(c"./")
}

fn open_tmp_in_tmpdir() -> c_int {
    open_tmp_in(tmpdir())
}

fn open_tmp_in(_dir: &CStr) -> c_int {
    cfg_select! {
        target_os = "linux" => unsafe {
            open(
                _dir.as_ptr(),
                O_RDWR | O_EXCL | libc::O_TMPFILE | libc::O_CLOEXEC,
                (S_IRUSR | S_IWUSR) as c_uint,
            )
        },
        _ => -1,
    }
}

fn open_named_in_tmpdir() -> c_int {
    unsafe {
        let id = random_id();
        let mut tmp = tmpdir().to_owned().into_bytes();

        tmp.extend_from_slice(format!("/closure-ffi-{id:016x}\0").as_bytes());
        let name_ptr = tmp.as_ptr() as *const c_char;

        let flags = cfg_select! {
            target_os = "linux" => O_RDWR | O_CREAT | O_EXCL | libc::O_CLOEXEC,
            _ => O_RDWR | O_CREAT | O_EXCL,
        };

        let fd = open(name_ptr, flags, (S_IRUSR | S_IWUSR) as c_uint);

        if fd != -1 {
            unlink(name_ptr);
        }

        fd
    }
}

fn tmpdir() -> &'static CStr {
    // Get TMPDIR or default to /tmp
    let env = unsafe { getenv(c"TMPDIR".as_ptr()) };
    if !env.is_null() {
        unsafe { CStr::from_ptr(env) }
    }
    else {
        c"/tmp"
    }
}

/// Builds a pseudo-random 64-bit ID by hashing a volatile state, stack address,
/// timestamps and current PID.
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

        getpid().hash(&mut hasher);
    }

    let hash = hasher.finish();

    STATE.fetch_add(hash as u32, Relaxed);

    hash
}
