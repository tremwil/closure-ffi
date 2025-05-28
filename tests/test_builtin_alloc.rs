#![cfg(feature = "bundled_jit_alloc")]

use closure_ffi::{traits::FnPtr, BareFn, BareFnMut, BareFnOnce};

#[test]
fn test_infer_closure() {
    let bare_closure: BareFn<'_, unsafe extern "C" fn(usize) -> u32>;
    bare_closure = BareFn::new(|arg| arg as _);

    assert_eq!(unsafe { bare_closure.bare().call((42,)) }, 42);
}

#[test]
fn test_boxed_fn() {
    let boxed_dyn = Box::new(|n: usize| 2 * n) as Box<dyn Fn(usize) -> usize>;
    let bare_closure = BareFn::new_c(boxed_dyn);

    assert_eq!(unsafe { bare_closure.bare().call((42,)) }, 84);
}

#[test]
fn test_ref_dyn() {
    let closure = |n: usize| 2 * n;
    let bare_closure = BareFn::new_c(&closure as &dyn Fn(usize) -> usize);

    assert_eq!(unsafe { bare_closure.bare().call((42,)) }, 84);
}

#[test]
fn test_mut_dyn() {
    let mut sum = 0;
    let mut closure = |n: usize| sum += n;

    let bare_closure = BareFnMut::new_c(&mut closure as &mut dyn FnMut(usize));
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
fn test_stateless_fn() {
    let bare_closure = BareFn::new_c(|n: usize| 2 * n);

    let bare = bare_closure.bare();
    assert_eq!(unsafe { bare(5) }, 10);
}

#[test]
fn test_borrow_fn() {
    let array = [0, 5, 10, 15, 20];
    let bare_closure = BareFn::new_c(|n| array[n]);

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
#[cfg(not(feature = "no_std"))]
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
#[cfg(not(feature = "no_std"))]
#[test]
fn test_print_fn_once() {
    let bare_closure = BareFnOnce::new_c(move |n: usize| {
        println!("{:08x}", n);
        3 * n
    });

    let bare = bare_closure.leak();
    assert_eq!(unsafe { bare(5) }, 15);
}
