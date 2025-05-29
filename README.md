Provides wrappers around closures which allows them to be called through context-free unsafe
bare functions.

Context-free bare functions are not needed very often, as properly designed C APIs typically
allow the user to specify an opaque pointer to a context object which will be provided to the
function pointer. However, this is not always the case, and may be impossible in less common
scenarios, e.g. function hooking for game modding/hacking.

# Features
The crate comes with the following feature flags:
- `no_std`: Makes the crate compatible with `#![no_std]`. A dependency on `alloc` and `spin` is
  still required.
- `bundled_jit_alloc`: Provides a global JIT allocator through the [`jit-allocator`](https://crates.io/crates/jit-allocator)
  crate. This is enabled by default.
- `custom_jit_alloc`: Allows providing a global JIT allocator through the `global_jit_alloc!` macro.
  **This is incompatible with `bundled_jit_alloc`**.
- `proc_macros`: Provides the `bare_hrtb` proc macro which is necessary for creating bare
  functions with signatures that involve higher-kinded lifetimes (i.e. `for<'a, ...>`
  statements), as well as the `bare_dyn` proc macro for writing `BareFn*` types of boxed 
  closures (i.e. `Box<dyn Fn()>`) more concisely. 
- `unstable`: Enable the use of unstable Rust features for aspects of the crate that benefit from 
  them. Currently, only 
- `full`: Enables `bundled_jit_alloc` and `proc_macros` features.

# Examples
Passing a closure to a C API taking a contextless function pointer:
```rust
use closure_ffi::{BareFnMut};
// Imagine we have an foreign C API for reigstering and unregistering some callback function.
// Notably, the API does not let the user provide a context object to the callback.
unsafe extern "C" fn ffi_register_callback(cb: unsafe extern "C" fn(u32)) {
    // ...
}
unsafe extern "C" fn ffi_unregister_callback(cb: unsafe extern "C" fn(u32)) {
    // ...
}

#[cfg(feature = "bundled_jit_alloc")]
{
    // We want to keep track of sum of callback arguments without using 
    // statics. This is where closure-ffi comes in:
    let mut sum = 0; // Non-'static closures work too!
    let wrapped = BareFnMut::new_c(|x: u32| {
        sum += x;
    });

    // Safety: Here, we assert that the foreign API won't use the callback
    // in ways that break Rust's safety rules. Namely:
    // - The exclusivity of the FnMut's borrow is respected.
    // - If the calls are made from a different thread, the closure is Sync.
    // - We unregister the callback before the BareFnMut is dropped.
    unsafe {
        ffi_register_callback(wrapped.bare());
        // Do something that triggers the callback...
        ffi_unregister_callback(wrapped.bare());
    }

    drop(wrapped);
    println!("{sum}");
}
```