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
