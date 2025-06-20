Provides wrappers around closures which allows them to be called through context-free unsafe
bare functions.

Context-free bare functions are not needed very often, as properly designed C APIs typically
allow the user to specify an opaque pointer to a context object which will be provided to the
function pointer. However, this is not always the case, and may be impossible in less common
scenarios, e.g. function hooking for game modding/hacking.

# Features
The crate comes with the following feature flags:

## Stable
- `std` (default): Use `std` features. When this is turned off, the crate is compatible with `no_std`,
  although a global allocator must be defined.
- `global_jit_alloc` (default): Provides the `GlobalJitAlloc` ZST which defers to a global JIT allocator implementation
  provided either through `default_jit_alloc` feature or the `global_jit_alloc!` macro.
- `default_jit_alloc` (default): Provides a global JIT allocator implementation through the 
  [`jit-allocator2`](https://crates.io/crates/jit-allocator2) crate. Note that said crate relies on operating system APIs,
  so while some `no_std` configurations are supported, bare metal ones will not be able to use this feature. 
- `proc_macros`: Provides the `bare_hrtb` proc macro which is necessary for creating bare
  functions with signatures that involve higher-kinded lifetimes (i.e. `for<'a, ...>` statements).

## Unstable (require a nightly compiler)
- `unstable`: Enable the use of unstable Rust features for aspects of the crate that benefit from 
  them without causing any API breaks. Unstable features that can cause breaking changes when enabled 
  are gated separately, and also enable this feature.
- `tuple_trait`: Adds a [`core::marker::Tuple`](https://doc.rust-lang.org/nightly/core/marker/trait.Tuple.html)
  bound on `FnPtr::Args`. This allows downstream crates to easily integrate the library with closure-related
  nightly features such as `unboxed_closures` and `fn_traits`.
- `c_variadic`: Adds *partial* (no invocation through `call`) `FnPtr` and `Fn*Thunk` implementations for variadic functions.
- `coverage`: Enables support for the `-C instrument-coverage` compiler flag.

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

#[cfg(feature = "default_jit_alloc")]
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

# Credits
- [tremwil](https://github.com/tremwil/): Library author and maintainer
- [Dasaav](https://github.com/Dasaav-dsv/): `lock (x14) push eax` x86 byte sequence idea