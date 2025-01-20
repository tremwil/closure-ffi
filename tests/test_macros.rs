use closure_ffi::{bare_dyn, cc, BareFn, BareFnMut};

#[test]
fn test_bare_dyn() {
    let bare_closure: bare_dyn!("system", FnMut(u32) -> u32 + Send + Sync) =
        BareFnMut::new_system(Box::new(|x| 2 * x));

    assert_eq!(unsafe { bare_closure.bare()(42) }, 84);
}

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

/// Ensure that cc::hrtb is actually required for [`test_hrtb`] to compile.
///
/// ```compile_fail
/// use closure_ffi::BareFn;
///
/// let bare_closure = BareFn::new_c(|opt: &Option<u32>| opt.as_ref());
///
/// fn takes_for_lt_bare_fn(bare_fn: unsafe extern "C" fn(&Option<u32>) -> Option<&u32>) {}
/// takes_for_lt_bare_fn(bare_closure.bare());
/// ```
#[allow(dead_code)]
fn assert_hrtb_required() {}
