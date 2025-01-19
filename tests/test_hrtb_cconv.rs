use closure_ffi::{bare_closure::bare_dyn, cc, BareFn};

type X = bare_dyn!("C", Send + Sync + for<'a> FnMut(&'a u32) -> &'a usize);

#[test]
fn test_hrtb() {
    let bare_closure = BareFn::new(
        cc::hrtb!(#[with(<T>)] for<'a> extern "C" fn(&'a Option<T>) -> Option<&'a T>),
        |opt| opt.as_ref(),
    );

    assert_eq!(unsafe { bare_closure.bare()(&Some(123)) }, Some(&123));
}
