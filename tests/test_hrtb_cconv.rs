//use closure_ffi_proc_macros::hrtb_cc;

//hrtb_cc!(#[with(<T> where X: Bleh)] for<'a> fn(&'a usize) -> usize);

// fn test(f: impl Fn(&usize) -> &usize) {
//     {
//         use ::closure_ffi::
//         struct _CustomThunk<F>(F);

//         unsafe impl<F: for<'a> FnOnce(&'a usize) -> &'a usize>
//             FnOnceThunk<for<'a> unsafe extern "C" fn(&'a usize) -> &'a usize> for _CustomThunk<F>
//         {
//             const THUNK_TEMPLATE_ONCE: for<'a> unsafe extern "C" fn(&'a usize) -> &'a usize = {
//                 unsafe extern "C" fn thunk<'a, F: for<'b> FnOnce(&'b usize) -> &'b usize>(
//                     arg: &'a usize,
//                 ) -> &'a usize {
//                     let closure_ptr: *mut F;
//                     crate::arch::thunk_asm!(closure_ptr);
//                     closure_ptr.read()(arg)
//                 }
//                 thunk::<F>
//             };
//         }

//         unsafe impl<F: for<'a> FnMut(&'a usize) -> &'a usize>
//             FnMutThunk<for<'a> unsafe extern "C" fn(&'a usize) -> &'a usize> for _CustomThunk<F>
//         {
//             const THUNK_TEMPLATE_MUT: for<'a> unsafe extern "C" fn(&'a usize) -> &'a usize = {
//                 unsafe extern "C" fn thunk<'a, F: for<'b> FnMut(&'b usize) -> &'b usize>(
//                     arg: &'a usize,
//                 ) -> &'a usize {
//                     let closure_ptr: *mut F;
//                     crate::arch::thunk_asm!(closure_ptr);
//                     (*closure_ptr)(arg)
//                 }
//                 thunk::<F>
//             };
//         }

//         unsafe impl<F: for<'a> Fn(&'a usize) -> &'a usize>
//             FnThunk<for<'a> unsafe extern "C" fn(&'a usize) -> &'a usize> for _CustomThunk<F>
//         {
//             const THUNK_TEMPLATE: for<'a> unsafe extern "C" fn(&'a usize) -> &'a usize = {
//                 unsafe extern "C" fn thunk<'a, F: for<'b> Fn(&'b usize) -> &'b usize>(
//                     arg: &'a usize,
//                 ) -> &'a usize {
//                     let closure_ptr: *const F;
//                     crate::arch::thunk_asm!(closure_ptr);
//                     (*closure_ptr)(arg)
//                 }
//                 thunk::<F>
//             };
//         }

//         _CustomThunk(f);
//     }
// }
