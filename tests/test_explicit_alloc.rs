mod slab_alloc;

#[allow(unused_imports)]
use closure_ffi::{cc, BareFn, BareFnMut, BareFnOnce, UntypedBareFn};
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
            println!("{n:08x}");
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
            println!("{n:08x}");
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
            println!("{n:08x}");
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

#[cfg(not(feature = "no_std"))]
#[test]
fn test_unwind_fn() {
    let capture = 42usize;
    let bare_closure = BareFn::with_cc_in(cc::CUnwind, |arg| assert_eq!(arg, capture), &SLAB);
    let bare = bare_closure.bare();

    // OK
    unsafe { bare(42) };

    // Panics, see if we unwind across the boundary
    let result = std::panic::catch_unwind(|| unsafe { bare(0) });
    assert!(result.is_err())
}

#[cfg(not(feature = "no_std"))]
#[test]
fn test_untyped_bare_fn() {
    use core::cell::Cell;

    // Use this type to verify that our closures were dropped
    #[derive(Debug)]
    struct SetOnDrop<'a>(&'a Cell<bool>);
    impl Drop for SetOnDrop<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum IntOrStr {
        Int(i32),
        Str(&'static str),
    }
    let shared = Cell::new(IntOrStr::Int(0));
    let shared_ref = &shared;

    let mut bare_closures = Vec::default();

    let int_closure_dropped = Cell::new(false);
    let int_closure_check = SetOnDrop(&int_closure_dropped);
    let int_closure = move |arg: i32| {
        let _ = &int_closure_check;
        shared_ref.set(IntOrStr::Int(arg));
    };

    bare_closures.push(BareFn::new_c_in(int_closure, &SLAB).into_untyped());

    let str_closure_dropped = Cell::new(false);
    let str_closure_check = SetOnDrop(&str_closure_dropped);
    let str_closure = move |arg: &'static str| {
        println!("{str_closure_check:?}"); // Move it into the closure
        shared_ref.set(IntOrStr::Str(arg));
    };
    bare_closures.push(BareFn::new_c_in(str_closure, &SLAB).into_untyped());

    unsafe {
        let takes_int: unsafe extern "C" fn(i32) = core::mem::transmute(bare_closures[0].bare());
        let takes_str: unsafe extern "C" fn(&'static str) =
            core::mem::transmute(bare_closures[1].bare());

        takes_str("foo");
        assert_eq!(shared.get(), IntOrStr::Str("foo"));

        takes_int(42);
        assert_eq!(shared.get(), IntOrStr::Int(42));

        takes_str("bar");
        assert_eq!(shared.get(), IntOrStr::Str("bar"));
    }

    drop(bare_closures);
    assert!(int_closure_dropped.get());
    assert!(str_closure_dropped.get());
}

#[test]
fn test_untyped_upcast() {
    use core::sync::atomic::{AtomicBool, Ordering::SeqCst};

    use closure_ffi::{traits::Any, BareFnSync};

    // Use this type to verify that our closure was dropped properly
    #[derive(Debug)]
    struct SetOnDrop<'a>(&'a AtomicBool);
    impl Drop for SetOnDrop<'_> {
        fn drop(&mut self) {
            self.0.store(true, SeqCst);
        }
    }

    let dropped = AtomicBool::new(false);
    let check = SetOnDrop(&dropped);

    let send_and_sync = BareFnSync::new_c_in(
        move || {
            let drop_flag = &check;
            drop_flag.0.load(SeqCst);
        },
        &SLAB,
    );

    // Upcast from a typed bare fn
    let untyped_send: UntypedBareFn<dyn Send, _> = send_and_sync.into();
    assert!(!dropped.load(SeqCst));

    // Upcast from an untyped bare fn
    let untyped_any: UntypedBareFn<dyn Any, _> = untyped_send.upcast();
    assert!(!dropped.load(SeqCst));

    unsafe {
        let bare: unsafe extern "C" fn() -> bool = core::mem::transmute(untyped_any.bare());
        assert!(!bare());
    }

    drop(untyped_any);
    assert!(dropped.load(SeqCst));
}
