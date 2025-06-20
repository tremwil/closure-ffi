#![cfg(all(feature = "global_jit_alloc", not(feature = "default_jit_alloc")))]

mod slab_alloc;

use closure_ffi::{global_jit_alloc, BareFn, BareFnMut};
use slab_alloc::SlabAlloc;

#[cfg(feature = "std")]
static SLAB: std::sync::LazyLock<SlabAlloc> = std::sync::LazyLock::new(|| SlabAlloc::new(0x10000));

#[cfg(not(feature = "std"))]
static SLAB: spin::Lazy<SlabAlloc> = spin::Lazy::new(|| SlabAlloc::new(0x10000));

global_jit_alloc!(SLAB);

#[test]
fn test_stateless_fn() {
    let bare_closure = BareFn::new_c(move |n: usize| 2 * n);

    let bare = bare_closure.bare();
    assert_eq!(unsafe { bare(5) }, 10);
}

#[test]
fn test_borrow_fn() {
    let array = [0, 5, 10, 15, 20];
    let bare_closure = BareFn::new_c(|n: usize| array[n]);

    let bare = bare_closure.bare();
    assert_eq!(unsafe { bare(3) }, 15);
}

#[test]
fn test_borrow_fn_mut() {
    let mut sum = 0;
    let bare_closure = BareFnMut::new_c(|n: usize| sum += n);
    let bare = bare_closure.bare();

    unsafe {
        bare(5);
        bare(3);
        bare(7);
    }

    drop(bare_closure);
    assert_eq!(sum, 15);
}

#[test]
fn test_moved_fn_mut() {
    let mut sum = 0;
    let bare_closure = BareFnMut::new_c(move |n: usize| {
        sum += n;
        sum
    });
    let bare = bare_closure.bare();

    unsafe {
        assert_eq!(bare(5), 5);
        assert_eq!(bare(3), 8);
        assert_eq!(bare(4), 12);
    }
}

// This used to segfault on A32/T32 targets:
// https://github.com/tremwil/closure-ffi/issues/3
#[cfg(feature = "std")]
#[test]
fn test_print_fn() {
    let bare_closure = BareFn::new_c(move |n: usize| {
        println!("{:08x}", n);
        3 * n
    });

    let bare = bare_closure.bare();
    assert_eq!(unsafe { bare(5) }, 15);
}

// This used to segfault on A32/T32 targets:
// https://github.com/tremwil/closure-ffi/issues/3
#[cfg(feature = "std")]
#[test]
fn test_print_fn_once() {
    use closure_ffi::BareFnOnce;

    let bare_closure = BareFnOnce::new_c(move |n: usize| {
        println!("{:08x}", n);
        3 * n
    });

    let bare = bare_closure.leak();
    assert_eq!(unsafe { bare(5) }, 15);
}
