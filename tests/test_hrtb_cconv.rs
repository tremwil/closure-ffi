use closure_ffi::{cc, BareFn};

#[test]
fn test_hrtb() {
    let bare_closure = BareFn::new(
        cc::hrtb!(#[with(<T>)] for<'a> extern "C" fn(&'a Option<T>) -> Option<&'a T>),
        |opt| opt.as_ref(),
    );

    assert_eq!(unsafe { bare_closure.bare()(&Some(123)) }, Some(&123));
}
