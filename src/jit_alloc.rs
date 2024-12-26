use jit_allocator::JitAllocator;

#[derive(Debug)]
pub struct JitAllocError;

pub trait JitAlloc {
    /// Allocates `size` bytes in the executable memory region.
    /// Returns two pointers. One points to Read-Execute mapping and another to Read-Write mapping.
    /// All code writes *must* go to the Read-Write mapping.
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError>;

    /// Releases the memory allocated by `alloc`.
    ///
    /// # SAFETY
    /// - `rx_ptr` must have been returned from `alloc`
    /// - `rx_ptr` must have been allocated from this allocator
    /// - `rx_ptr` must not have been passed to `release` before
    /// - `rx_ptr` must point to read-execute part of memory returned from `alloc`.
    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError>;
}

impl JitAlloc for core::cell::RefCell<JitAllocator> {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        self.borrow_mut().alloc(size).map_err(|_| JitAllocError)
    }

    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
        self.borrow_mut().release(rx_ptr).map_err(|_| JitAllocError)
    }
}

#[cfg(not(feature = "no_std"))]
impl JitAlloc for std::sync::RwLock<JitAllocator> {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        self.write().unwrap().alloc(size).map_err(|_| JitAllocError)
    }

    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
        self.write().unwrap().release(rx_ptr).map_err(|_| JitAllocError)
    }
}

#[cfg(not(feature = "no_std"))]
impl JitAlloc for std::sync::Mutex<JitAllocator> {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        self.lock().unwrap().alloc(size).map_err(|_| JitAllocError)
    }

    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
        self.lock().unwrap().release(rx_ptr).map_err(|_| JitAllocError)
    }
}

impl<'a, J: JitAlloc> JitAlloc for &'a J {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        (*self).alloc(size)
    }

    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
        (*self).release(rx_ptr)
    }
}

#[cfg(feature = "no_std")]
static GLOBAL_JIT_ALLOC: spin::Mutex<Option<alloc::boxed::Box<JitAllocator>>> =
    spin::Mutex::new(None);
#[cfg(not(feature = "no_std"))]
static GLOBAL_JIT_ALLOC: std::sync::Mutex<Option<Box<JitAllocator>>> = std::sync::Mutex::new(None);

/// The default, global JIT allocator.
///
/// This is currently implemented as a ZST deffering to a static [`jit_allocator::JitAllocator`]
/// behind a [`std::sync::Mutex`] (or a [`spin::Mutex`] under no_std).
#[derive(Default, Clone, Copy)]
pub struct GlobalJitAlloc;

impl GlobalJitAlloc {
    fn use_alloc<T>(&self, action: impl FnOnce(&mut JitAllocator) -> T) -> T {
        #[cfg(feature = "no_std")]
        let mut maybe_alloc = GLOBAL_JIT_ALLOC.lock();
        #[cfg(not(feature = "no_std"))]
        let mut maybe_alloc = GLOBAL_JIT_ALLOC.lock().unwrap();

        let mut alloc = maybe_alloc.get_or_insert_with(|| JitAllocator::new(Default::default()));
        action(&mut alloc)
    }
}

impl JitAlloc for GlobalJitAlloc {
    fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), JitAllocError> {
        self.use_alloc(|a| a.alloc(size)).map_err(|_| JitAllocError)
    }

    unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), JitAllocError> {
        self.use_alloc(|a| a.release(rx_ptr)).map_err(|_| JitAllocError)
    }
}

#[cfg(not(feature = "no_std"))]
mod thread_jit_alloc {
    #[allow(unused_imports)]
    use super::GlobalJitAlloc;

    use core::{cell::UnsafeCell, marker::PhantomData};

    use jit_allocator::JitAllocator;

    thread_local! {
        static THREAD_JIT_ALLOC: UnsafeCell<Box<JitAllocator>> =
            UnsafeCell::new(JitAllocator::new(Default::default()));
    }

    /// Marker type providing access to a thread-local JIT allocator.
    ///
    /// Unlike [`GlobalJitAlloc`], this allocator is neither [`Send`] nor [`Sync`], so the
    /// following does not compile:
    ///
    /// ```compile_fail
    /// fn takes_send(_: impl Send) {}
    /// takes_send(ThreadJitAlloc::default());
    /// ```
    ///
    /// ```compile_fail
    /// fn takes_sync(_: impl Sync) {}
    /// takes_sync(ThreadJitAlloc::default());
    /// ```
    #[derive(Default, Clone, Copy)]
    pub struct ThreadJitAlloc(PhantomData<*mut ()>);

    impl super::JitAlloc for ThreadJitAlloc {
        fn alloc(&self, size: usize) -> Result<(*const u8, *mut u8), super::JitAllocError> {
            THREAD_JIT_ALLOC
                .with(|a| unsafe { &mut *a.get() }.alloc(size))
                .map_err(|_| super::JitAllocError)
        }

        unsafe fn release(&self, rx_ptr: *const u8) -> Result<(), super::JitAllocError> {
            THREAD_JIT_ALLOC
                .with(|a| unsafe { &mut *a.get() }.release(rx_ptr))
                .map_err(|_| super::JitAllocError)
        }
    }
}
#[cfg(not(feature = "no_std"))]
pub use thread_jit_alloc::*;
