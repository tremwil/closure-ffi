//! Provides factory functions for creating [`FnThunk`] implementations from a closure while
//! preserving its [Sync]/[Send]ness.

use crate::traits::{FnMutThunk, FnOnceThunk, FnPtr, FnThunk, PackedFn, PackedFnMut, PackedFnOnce};

// SAFETY: Using the `SendSyncWrapper` type below is only unsound if `FnPtr::make_thunk` and friends
// do not preserve the send/syncness of `fun` in the generated thunks. Since the opaque thunk impl
// is merely the combination of a ZST calling convention marker type and the same closure with
// expanded parameters, the marker traits are preserved.

/// Creates a [`FnOnceThunk`] implementation from a closure taking the same arguments as this
/// function pointer.
///
/// This is identical to [`FnPtr::make_once_thunk`] and is only provided for convenience.
#[inline(always)]
pub fn make_once<B: FnPtr, F>(fun: F) -> impl FnOnceThunk<B>
where
    F: Send + for<'x, 'y, 'z> PackedFnOnce<'x, 'y, 'z, B>,
{
    B::make_once_thunk(fun)
}

/// Creates a [Send] [`FnOnceThunk`] implementation from a closure taking the same arguments as
/// this function pointer.
#[inline(always)]
pub fn make_once_send<B: FnPtr, F>(fun: F) -> impl FnOnceThunk<B> + Send
where
    F: Send + for<'x, 'y, 'z> PackedFnOnce<'x, 'y, 'z, B>,
{
    let thunk = B::make_once_thunk(fun);
    SendSyncWrapper(thunk)
}

/// Creates a [Sync] [`FnOnceThunk`] implementation from a closure taking the same arguments as
/// this function pointer.
#[inline(always)]
pub fn make_once_sync<B: FnPtr, F>(fun: F) -> impl FnOnceThunk<B> + Sync
where
    F: Sync + for<'x, 'y, 'z> PackedFnOnce<'x, 'y, 'z, B>,
{
    let thunk = B::make_once_thunk(fun);
    SendSyncWrapper(thunk)
}

/// Creates a [Send] and [Sync] [`FnOnceThunk`] implementation from a closure taking the same
/// arguments as this function pointer.
#[inline(always)]
pub fn make_once_send_sync<B: FnPtr, F>(fun: F) -> impl FnOnceThunk<B> + Send + Sync
where
    F: Send + Sync + for<'x, 'y, 'z> PackedFnOnce<'x, 'y, 'z, B>,
{
    let thunk = B::make_once_thunk(fun);
    SendSyncWrapper(thunk)
}

/// Creates a [`FnMutThunk`] implementation from a closure taking the same arguments as this
/// function pointer.
///
/// This is identical to [`FnPtr::make_mut_thunk`] and is only provided for convenience.
#[inline(always)]
pub fn make_mut<B: FnPtr, F>(fun: F) -> impl FnMutThunk<B>
where
    F: for<'x, 'y, 'z> PackedFnMut<'x, 'y, 'z, B>,
{
    B::make_mut_thunk(fun)
}

/// Creates a [Send] [`FnMutThunk`] implementation from a closure taking the same arguments as
/// this function pointer.
#[inline(always)]
pub fn make_mut_send<B: FnPtr, F>(fun: F) -> impl FnMutThunk<B> + Send
where
    F: Send + for<'x, 'y, 'z> PackedFnMut<'x, 'y, 'z, B>,
{
    let thunk = B::make_mut_thunk(fun);
    SendSyncWrapper(thunk)
}

/// Creates a [Sync] [`FnMutThunk`] implementation from a closure taking the same arguments as
/// this function pointer.
#[inline(always)]
pub fn make_mut_sync<B: FnPtr, F>(fun: F) -> impl FnMutThunk<B> + Sync
where
    F: Sync + for<'x, 'y, 'z> PackedFnMut<'x, 'y, 'z, B>,
{
    let thunk = B::make_mut_thunk(fun);
    SendSyncWrapper(thunk)
}

/// Creates a [Send] and [Sync] [`FnMutThunk`] implementation from a closure taking the same
/// arguments as this function pointer.
#[inline(always)]
pub fn make_mut_send_sync<B: FnPtr, F>(fun: F) -> impl FnMutThunk<B> + Send + Sync
where
    F: Send + Sync + for<'x, 'y, 'z> PackedFnMut<'x, 'y, 'z, B>,
{
    let thunk = B::make_mut_thunk(fun);
    SendSyncWrapper(thunk)
}

/// Creates a [`FnThunk`] implementation from a closure taking the same arguments as this
/// function pointer.
///
/// This is identical to [`FnPtr::make_thunk`] and is only provided for convenience.
#[inline(always)]
pub fn make<B: FnPtr, F>(fun: F) -> impl FnThunk<B>
where
    F: for<'x, 'y, 'z> PackedFn<'x, 'y, 'z, B>,
{
    B::make_thunk(fun)
}
/// Creates a [Send] [`FnThunk`] implementation from a closure taking the same arguments as
/// this function pointer.
#[inline(always)]
pub fn make_send<B: FnPtr, F>(fun: F) -> impl FnThunk<B> + Send
where
    F: Send + for<'x, 'y, 'z> PackedFn<'x, 'y, 'z, B>,
{
    let thunk = B::make_thunk(fun);
    SendSyncWrapper(thunk)
}

/// Creates a [Sync] [`FnThunk`] implementation from a closure taking the same arguments as
/// this function pointer.
#[inline(always)]
pub fn make_sync<B: FnPtr, F>(fun: F) -> impl FnThunk<B> + Sync
where
    F: Sync + for<'x, 'y, 'z> PackedFn<'x, 'y, 'z, B>,
{
    let thunk = B::make_thunk(fun);
    SendSyncWrapper(thunk)
}

/// Creates a [Send] and [Sync] [`FnThunk`] implementation from a closure taking the same
/// arguments as this function pointer.
#[inline(always)]
pub fn make_send_sync<B: FnPtr, F>(fun: F) -> impl FnThunk<B> + Send + Sync
where
    F: Send + Sync + for<'x, 'y, 'z> PackedFn<'x, 'y, 'z, B>,
{
    let thunk = B::make_thunk(fun);
    SendSyncWrapper(thunk)
}

#[repr(transparent)]
struct SendSyncWrapper<T>(T);
unsafe impl<T> Send for SendSyncWrapper<T> {}
unsafe impl<T> Sync for SendSyncWrapper<T> {}
unsafe impl<B: FnPtr, T: FnOnceThunk<B>> FnOnceThunk<B> for SendSyncWrapper<T> {
    const THUNK_TEMPLATE_ONCE: *const u8 = T::THUNK_TEMPLATE_ONCE;
    unsafe fn call_once<'a, 'b, 'c>(
        self,
        args: <B as FnPtr>::Args<'a, 'b, 'c>,
    ) -> <B as FnPtr>::Ret<'a, 'b, 'c> {
        self.0.call_once(args)
    }
}
unsafe impl<B: FnPtr, T: FnMutThunk<B>> FnMutThunk<B> for SendSyncWrapper<T> {
    const THUNK_TEMPLATE_MUT: *const u8 = T::THUNK_TEMPLATE_MUT;
    unsafe fn call_mut<'a, 'b, 'c>(
        &mut self,
        args: <B as FnPtr>::Args<'a, 'b, 'c>,
    ) -> <B as FnPtr>::Ret<'a, 'b, 'c> {
        self.0.call_mut(args)
    }
}
unsafe impl<B: FnPtr, T: FnThunk<B>> FnThunk<B> for SendSyncWrapper<T> {
    const THUNK_TEMPLATE: *const u8 = T::THUNK_TEMPLATE;
    unsafe fn call<'a, 'b, 'c>(
        &self,
        args: <B as FnPtr>::Args<'a, 'b, 'c>,
    ) -> <B as FnPtr>::Ret<'a, 'b, 'c> {
        self.0.call(args)
    }
}
