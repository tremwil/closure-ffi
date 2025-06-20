#![cfg(all(feature = "proc_macros", feature = "default_jit_alloc"))]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(improper_ctypes_definitions)]

use closure_ffi::{bare_closure::bare_hrtb, BareFn, BareFnMut};

bare_hrtb! {
    type MyFn = for<'a, 'b> extern "C" fn(&'a str, &'b str) -> &'a str;
}

// Derefs to a `&str` whose lifetime is bounded. Goal is to prevent static promotion to make these
// tests really ensure the lifetime is different
struct MyStr(&'static str);
impl core::ops::Deref for MyStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[test]
fn test_basic_hrtb() {
    let ignore_y = BareFn::<MyFn>::new(|x, _y| x);

    fn do_test(bare_fn: for<'a, 'b> unsafe extern "C" fn(&'a str, &'b str) -> &'a str) {
        let foo = MyStr("foo");
        let result = {
            let bar = MyStr("bar");
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
    const STATIC_STR: &str = "xyz";
    let owned_str1 = MyStr("foo");
    let owned_str2 = MyStr("bar");

    let mut ref_str: &str = &owned_str1;

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
    assert_eq!(ref_str, &*owned_str2);
}

bare_hrtb! {
    type MyGenericFn<T> where T: Clone = for<'a> extern "C" fn(&'a Option<T>) -> Option<&'a T>;
}

#[test]
fn test_generic_hrtb() {
    // alternatively: let bare_closure = BareFn::with_cc(MyGenericFn_CC, |opt| opt.as_ref());
    let bare_closure = BareFn::<MyGenericFn<_>>::new(|opt| opt.as_ref());

    // Ensure that the function is truly lifetime generic (this would fail without cc::hrtb)
    fn do_test(bare_fn: unsafe extern "C" fn(&Option<u32>) -> Option<&u32>) {
        assert_eq!(unsafe { bare_fn(&Some(123)) }, Some(&123));
    }

    do_test(bare_closure.bare().into());
}
