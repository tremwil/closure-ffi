#![cfg(all(feature = "proc_macros", feature = "bundled_jit_alloc"))]
#![cfg_attr(feature = "no_std", no_std)]
#![allow(improper_ctypes_definitions)]

#[cfg(feature = "no_std")]
extern crate alloc;
#[cfg(feature = "no_std")]
use alloc::boxed::Box;

use closure_ffi::{bare_closure::bare_hrtb, BareFn, BareFnMut};

bare_hrtb! {
    type MyFn = for<'a, 'b> extern "C" fn(&'a str, &'b str) -> &'a str;
}

#[test]
fn test_basic_hrtb() {
    let ignore_y = BareFn::<MyFn>::new(|x, _y| x);

    fn do_test(bare_fn: for<'a, 'b> unsafe extern "C" fn(&'a str, &'b str) -> &'a str) {
        let foo = "foo".to_string();
        let result = {
            let bar = "bar".to_string();
            unsafe { bare_fn(&foo, &bar) }
        };
        assert_eq!(result, "foo");
    }

    do_test(ignore_y.bare().0);
}

bare_hrtb! {
    type MyFnLt<'c> = for<'a> extern "C" fn(&'c str, &'a u32) -> &'a u32;
}

#[test]
fn test_non_static_hrtb() {
    const STATIC_STR: &'static str = "xyz";
    let owned_str1 = "foo".to_string();
    let owned_str2 = "bar".to_string();

    let mut ref_str = owned_str1.as_str();

    let bare_closure = BareFnMut::<MyFnLt>::new(|x, y| {
        ref_str = x;
        y
    });
    let bare = bare_closure.bare();

    // Call with different lifetimes
    assert_eq!(unsafe { bare(STATIC_STR, &0) }, &0);
    {
        let num = 1;
        assert_eq!(unsafe { bare(&owned_str2, &num) }, &1);
    }

    drop(bare_closure);
    assert_eq!(ref_str, owned_str2);
}

bare_hrtb! {
    type MyGenericFn<T> where T: Clone = for<'a> extern "C" fn(&'a Option<T>) -> Option<&'a T>;
}

#[test]
fn test_generic_hrtb() {
    let bare_closure = BareFn::<MyGenericFn<_>>::new(|opt| opt.as_ref());

    // Ensure that the function is truly lifetime generic (this would fail without cc::hrtb)
    fn do_test(bare_fn: unsafe extern "C" fn(&Option<u32>) -> Option<&u32>) {
        assert_eq!(unsafe { bare_fn(&Some(123)) }, Some(&123));
    }

    do_test(bare_closure.bare().into());
}
