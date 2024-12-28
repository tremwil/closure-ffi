// use closure_ffi::{cc, thunk::FnMutThunk};

// fn into_bare<C, B: Copy, F>(_cconv: C, _fun: F) -> B
// where
//     (C, F): FnMutThunk<C, B>,
// {
//     todo!()
// }

// fn takes_bare(_: unsafe extern "C" fn(usize) -> i32) {}
// fn takes_hrtb<T>(_: unsafe extern "C" fn(&Option<T>) -> Option<&T>) {}

// fn test() {
//     takes_hrtb(into_bare(
//         cc::hrtb!(unsafe extern "C" fn(&Option<()>) -> Option<&()>),
//         |a| a.as_ref(),
//     ));

//     takes_bare(into_bare(cc::C, |i| i as i32));
// }
