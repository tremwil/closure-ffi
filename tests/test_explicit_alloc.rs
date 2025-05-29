mod slab_alloc;

#[allow(unused_imports)]
use closure_ffi::{BareFn, BareFnMut, BareFnOnce};
use slab_alloc::SlabAlloc;

#[cfg(not(feature = "no_std"))]
static SLAB: std::sync::LazyLock<SlabAlloc> = std::sync::LazyLock::new(|| SlabAlloc::new(0x10000));

#[cfg(feature = "no_std")]
static SLAB: spin::Lazy<SlabAlloc> = spin::Lazy::new(|| SlabAlloc::new(0x10000));

#[test]
fn test_stateless_fn() {
    let bare_closure = BareFn::new_c_in(move |n: usize| 2 * n, &SLAB);

    let bare = bare_closure.bare();
    assert_eq!(unsafe { bare(5) }, 10);
}

#[test]
fn test_borrow_fn() {
    let array = [0, 5, 10, 15, 20];
    let bare_closure = BareFn::new_c_in(
        |n: usize| {
            println!("{:08x}", n);
            array[n]
        },
        &SLAB,
    );

    let bare = bare_closure.bare();
    assert_eq!(unsafe { bare(3) }, 15);
}

#[test]
fn test_borrow_fn_mut() {
    let mut sum = 0;
    let bare_closure = BareFnMut::new_c_in(|n: usize| sum += n, &SLAB);
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
    let bare_closure = BareFnMut::new_c_in(
        move |n: usize| {
            sum += n;
            sum
        },
        &SLAB,
    );
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
    let bare_closure = BareFn::new_c_in(
        move |n: usize| {
            println!("{:08x}", n);
            3 * n
        },
        &SLAB,
    );

    let bare = bare_closure.bare();
    assert_eq!(unsafe { bare(5) }, 15);
}

// This used to segfault on A32/T32 targets:
// https://github.com/tremwil/closure-ffi/issues/3
#[cfg(not(feature = "no_std"))]
#[test]
fn test_print_fn_once() {
    let bare_closure = BareFnOnce::new_c_in(
        move |n: usize| {
            println!("{:08x}", n);
            3 * n
        },
        &SLAB,
    );

    let bare = bare_closure.leak();
    assert_eq!(unsafe { bare(5) }, 15);
}

#[cfg(not(feature = "no_std"))]
#[test]
fn test_double_free() {
    println!("test BareFn (coerced)");

    #[derive(Debug)]
    #[allow(dead_code)]
    struct Val(usize, Box<usize>);
    impl Val {
        fn new(val: usize) -> Self {
            Self(val, Box::new(101 /* just to alloc and drop */))
        }
    }
    impl Drop for Val {
        fn drop(&mut self) {
            println!("Val dropped (val={})", self.0);
        }
    }

    fn f() -> u32 {
        println!("bare BareFn call");
        let mut v = Val::new(100);
        v.0 += 1;
        drop(v);
        42
    }
    let f: fn() -> u32 = f as _; // <- impportant, ty-coercion

    // call FnItem as-is:
    assert_eq!(42, f());

    // wrap, deploy, call and drop:
    unsafe {
        let bare = BareFn::new_c_in(f, &SLAB);
        assert_eq!(42, bare.bare()());
    }

    // wrap, deploy, call and drop again:
    unsafe {
        let wrap = BareFn::new_c_in(f, &SLAB);
        let bare = wrap.bare();
        assert_eq!(42, bare());
        assert_eq!(42, bare());
    }

    // call FnItem as-is again:
    assert_eq!(42, f());
    println!("test FnPtr - done");
}
