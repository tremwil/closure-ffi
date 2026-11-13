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

    BackingFdProvider::with_global(|fd_provider| unsafe { fd_provider.mmap_rx_rw(size, 0) })
}

#[derive(Debug)]
struct BackingFdProvider {
    last_fd: Option<BackingFd>,
    last_strategy: usize,
    strategies: &'static [OpenStrategy],
}

#[derive(Debug)]
enum BackingFd {
    Resizable { fd: c_int, offset: usize },
    FixedSize { fd: c_int },
    Anonymous,
}

type OpenStrategy = fn(size: usize) -> ControlFlow<BackingFd>;

impl BackingFdProvider {
    fn with_global<T>(action: impl FnOnce(&mut Self) -> T) -> T {
        let mut provider = cfg_select! {
            feature = "std" => {{
                static FD: std::sync::Mutex<BackingFdProvider> =
                    std::sync::Mutex::new(BackingFdProvider::new());
                FD.lock().unwrap()
            }}
            _ => {{
                static FD: spin::Mutex<BackingFdProvider> =
                    spin::Mutex::new(BackingFdProvider::new());
                FD.lock()
            }}
        };

        action(&mut provider)
    }

    /// Creates a provider with multiple platform and feature flag dependent strategies
    /// for opening a fd that can be mapped as executable.
    ///
    /// The first one to succeed is memoized, failed strategies are discarded.
    const fn new() -> Self {
        /// Fixed size fd (memfd, shm, Android shared memory).
        fn fixed(fd: c_int) -> BackingFd {
            BackingFd::FixedSize { fd }
        }

        /// Resizable fd (file backed).
        fn resizable(fd: c_int) -> BackingFd {
            BackingFd::Resizable { fd, offset: 0 }
        }

        // 1. `memfd_create` (Linux and Android only)
        // 2. `ASharedMemory_create` (Android only)
        // 3. `shm_open` with `SHM_ANON` (FreeBSD only)
        // 4. anonymous RWX mapping
        //
        // Target feature "shm_open_jit_alloc" only:
        // 5. `shm_open` with a randomly assigned name
        //
        // Target feature "tmp_file_jit_alloc" only:
        // 6. `tmpfile` into `fileno`
        // 7. `open` with `O_TMPFILE` in the CWD (Linux only)
        // 8. `open` with `O_TMPFILE` in $TMPDIR or /tmp (Linux only)
        // 9. `open` in $TMPDIR or /tmp with a randomly assigned name
        let strategies: &'static [OpenStrategy] = &[
            #[cfg(any(target_os = "linux", target_os = "android"))]
            |size| BackingFd::try_map_resize(size, memfd_create(), fixed),
            #[cfg(all(target_os = "android", feature = "asharedmem_jit_alloc"))]
            |size| BackingFd::try_map_resize(size, asharedmem_create(size), fixed),
            #[cfg(all(target_os = "freebsd", feature = "shm_open_jit_alloc"))]
            |size| BackingFd::try_map_resize(size, shm_open_anon(), fixed),
            |size| BackingFd::try_map_anon(size),
            #[cfg(all(
                feature = "shm_open_jit_alloc",
                not(target_vendor = "apple"),
                not(target_os = "android"),
            ))]
            |size| BackingFd::try_map_resize(size, shm_open(), fixed),
            #[cfg(feature = "tmp_file_jit_alloc")]
            |size| BackingFd::try_map_resize(size, tmpfile_fileno(), resizable),
            #[cfg(all(feature = "tmp_file_jit_alloc", target_os = "linux"))]
            |size| BackingFd::try_map_resize(size, open_o_tmpfile_in(c"./"), resizable),
            #[cfg(all(feature = "tmp_file_jit_alloc", target_os = "linux"))]
            |size| BackingFd::try_map_resize(size, open_o_tmpfile_in(tmpdir()), resizable),
            #[cfg(feature = "tmp_file_jit_alloc")]
            |size| BackingFd::try_map_resize(size, open_in_tmpdir(), resizable),
        ];

        Self {
            last_fd: None,
            last_strategy: 0,
            strategies,
        }
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
        if self.last_fd.is_none() {
            // This is first time initialization or the last fd was discarded.
            // Need a new backing fd.
            self.last_fd = self.new_fd(size).break_value();
        }

        // Take the fd so it is discarded on fail (or if it's fixed size).
        let mut fd = self.last_fd.take()?;
        let rx_rw = fd.mmap_rx_rw(size, flags)?;

        // Fixed size fd are mapped fully and can't be reused.
        if !matches!(fd, BackingFd::FixedSize { .. }) {
            self.last_fd = Some(fd);
        }

        Some(rx_rw)
    }

    /// Goes through available fd opening strategies in order attempting to open a fd
    /// that allows for executable memory.
    ///
    /// The first strategy to succeed is memoized, failed strategies are discarded.
    ///
    /// # Safety
    ///
    /// `size` must be a multiple of the value returned by [`map_granularity`].
    unsafe fn new_fd(&mut self, size: usize) -> ControlFlow<BackingFd> {
        for strategy in &self.strategies[self.last_strategy..] {
            strategy(size)?;

            // Don't try this strategy again.
            self.last_strategy += 1;
        }

        ControlFlow::Continue(())
    }
}

impl BackingFd {
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

        let (fd, offset) = match self {
            Self::Resizable { fd, offset } => {
                let cur_offset = off_t::try_from(*offset).ok()?;
                let new_size = offset.checked_add(size)?;

                // Grow the file by `size` to map the newly allocated memory.
                if unsafe { ftruncate(*fd, new_size.try_into().ok()?) != 0 } {
                    return None;
                };

                *offset = new_size;

                (*fd, cur_offset)
            }
            Self::FixedSize { fd } => (*fd, 0),
            Self::Anonymous => return mmap_rwx(len, flags, -1, 0),
        };

        cfg_select! {
            // On macOS return a single mapping with the MAP_JIT flag.
            target_os = "macos" => mmap_rwx(len, flags, self.fd, offset),
            // On other platforms return a double mapping.
            _ => {
                let flags = flags | MAP_SHARED;

                let rx = unsafe { mmap(ptr::null_mut(), len, PROT_RX, flags, fd, offset) };

                if rx == MAP_FAILED {
                    return None;
                }

                let rw = unsafe { mmap(ptr::null_mut(), len, PROT_RW, flags, fd, offset) };

                if rw == MAP_FAILED {
                    munmap(rx, len);
                    return None;
                }

                Some((rx as *const u8, rw as *mut u8))
            }
        }
    }

    /// Tries mapping a fd as executable memory. A security policy might disallow this
    /// for example when /dev/shm or /tmp are mounted as noexec.
    fn try_map(size: size_t, fd: c_int, f: impl FnOnce(c_int) -> Self) -> ControlFlow<Self> {
        unsafe {
            if fd == -1 {
                return ControlFlow::Continue(());
            }

            let (prot, flags) = cfg_select! {
                target_os = "macos" => (PROT_RWX, MAP_PRIVATE | libc::MAP_JIT),
                _ => (PROT_RX, MAP_SHARED),
            };

            let rx = mmap(ptr::null_mut(), size, prot, flags, fd, 0);

            if rx == MAP_FAILED {
                return ControlFlow::Continue(());
            }

            munmap(rx, size);

            ControlFlow::Break(f(fd))
        }
    }

    /// Tries mapping a resizable fd as executable memory.
    ///
    /// For memfd, shm and Android shared memory this may be the first and last allowed resize.
    fn try_map_resize(size: size_t, fd: c_int, f: impl FnOnce(c_int) -> Self) -> ControlFlow<Self> {
        unsafe {
            if fd != -1 {
                if ftruncate(fd, size as off_t) != -1 {
                    Self::try_map(size, fd, f)?;
                }

                close(fd);
            }

            ControlFlow::Continue(())
        }
    }

    /// Tries mapping anonymous read-write executable memory.
    fn try_map_anon(size: size_t) -> ControlFlow<Self> {
        unsafe {
            let flags = cfg_select! {
                target_os = "macos" => MAP_PRIVATE | MAP_ANONYMOUS | libc::MAP_JIT,
                _ => MAP_PRIVATE | MAP_ANONYMOUS,
            };

            let rx = mmap(ptr::null_mut(), size, PROT_RWX, flags, -1, 0);

            if rx == MAP_FAILED {
                return ControlFlow::Continue(());
            }

            munmap(rx, size);

            ControlFlow::Break(Self::Anonymous)
        }
    }
}

impl Drop for BackingFd {
    fn drop(&mut self) {
        let (Self::Resizable { fd, .. } | Self::FixedSize { fd, .. }) = self
        else {
            return;
        };

        let fd = *fd;
        if fd < 0 {
            return;
        }

        unsafe {
            close(fd);
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn memfd_create() -> c_int {
    unsafe {
        // N.B. the name used with `memfd_create` does not have to be unique.
        // Use a syscall in case `memfd_create` is not implemented.
        libc::syscall(
            libc::SYS_memfd_create,
            c"closure-ffi".as_ptr(),
            libc::MFD_CLOEXEC,
        ) as c_int
    }
}

#[cfg(all(target_os = "android", feature = "asharedmem_jit_alloc"))]
fn asharedmem_create(_size: usize) -> c_int {
    unsafe {
        let res = ndk_sys::ASharedMemory_create(ptr::null(), _size);
        if res < 0 {
            return -1;
        }
        res
    }
}

#[cfg(target_os = "freebsd")]
fn shm_open_anon() -> c_int {
    unsafe { libc::shm_open(libc::SHM_ANON, O_RDWR | O_CREAT | O_EXCL, S_IRUSR | S_IWUSR) }
}

#[cfg(not(any(target_vendor = "apple", target_os = "android")))]
fn shm_open() -> c_int {
    unsafe {
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
    }
}

fn tmpfile_fileno() -> c_int {
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

#[cfg(target_os = "linux")]
fn open_o_tmpfile_in(_dir: &CStr) -> c_int {
    unsafe {
        open(
            _dir.as_ptr(),
            O_RDWR | O_EXCL | libc::O_TMPFILE | libc::O_CLOEXEC,
            (S_IRUSR | S_IWUSR) as c_uint,
        )
    }
}

fn open_in_tmpdir() -> c_int {
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
