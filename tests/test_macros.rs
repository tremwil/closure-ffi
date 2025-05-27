#![cfg(all(feature = "proc_macros", feature = "bundled_jit_alloc"))]
#![cfg_attr(feature = "no_std", no_std)]

#[cfg(feature = "no_std")]
extern crate alloc;
#[cfg(feature = "no_std")]
use alloc::boxed::Box;

use closure_ffi::{cc, BareFn, BareFnMut};

#[test]
fn test_hrtb() {
    let bare_closure = BareFn::new(
        cc::hrtb!(#[with(<T>)] for<'a> extern "C" fn(&'a Option<T>) -> Option<&'a T>),
        |opt| opt.as_ref(),
    );

    // Ensure that the function is truly lifetime generic (this would fail without cc::hrtb)
    fn do_test(bare_fn: unsafe extern "C" fn(&Option<u32>) -> Option<&u32>) {
        assert_eq!(unsafe { bare_fn(&Some(123)) }, Some(&123));
    }

    do_test(bare_closure.bare());
}
