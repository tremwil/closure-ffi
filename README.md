Provides wrappers around closures which allows them to be called through context-free unsafe
bare functions.

Context-free bare functions are not needed very often, as properly designed C APIs typically
allow the user to specify an opaque pointer to a context object which will be provided to the
function pointer. However, this is not always the case, and may be impossible in less common
scenarios, e.g. function hooking for game modding/hacking.

Currently supports the following platforms:
- Operating systems: Linux, Windows and OSX
- CPU Architectures: x86, x86_64, aarch64 and ARM v7+ (thumb and wide ISAs)

# Warning: UB ahead, avoid critical/production use!

<div class="warning">

The technique used by this crate to generate bare function thunks that invoke a closure leverages the compiler itself to emit the argument forwarding code at compile-time and is thus much simpler and faster than full-JIT approaches like [libffi](https://github.com/libffi/libffi). The major downside is that it relies on assumptions about the structure of the machine code emitted by the compiler, namely that the thunk's prologue can be trivially relocated. While reasonable for the supported architectures, these assumptions are not *guarantees* --- the crate relies on a form[^1] of [undefined behavior](https://doc.rust-lang.org/reference/behavior-considered-undefined.html).

[^1]: The kind of undefined behavior relied on by closure-ffi is not as bad as UB within the Rust abstract machine, which makes *your entire program* meaningless. The potentially incorrect codegen is restricted to JIT'ed thunks, and can only occur when one is actually called. 

Extensive testing with different bare function argument types (including non-FFI safe types!) has been conducted to ensure that the conditions for creating a broken thunk don't occur in practice, and any such breakage is extremely unlikely to manifest as anything more subtle than an immediate `SIGSEGV` on call. But in the end, **UB is still UB**.

If you are building production-grade or mission-critical software, please do not use this crate and rely on tried and tested solutions like [libffi](https://crates.io/crates/libffi) instead. It is mainly intended for game modding, process hacking and function hooks where strict soundness guarantees are not as important.

</div>

# Why closure-ffi
If the above warning hasn't dissuaded you from using this crate, here are the pros of closure-ffi:
- Supports a wide variety of functions types and calling conventions: 
    - Functions of up to 12 arguments with arbitrary argument types. This means that *all* ffi-safe types can be used in the function signature: thin references, `#[repr(C)]` types, `Option<&T>`, `NonNull`, [thin `CStr`](https://crates.io/crates/thin_cstr) refs, etc.
    - Lifetime-generic (a.k.a. higher-kinded) bare functions, e.g. `for<'a, 'b> unsafe extern "C" fn(&'a CStr, &'b CStr) -> &'a CStr` using the `bare_hrtb!` macro (requires the `proc_macros` feature)
    - Variadic functions e.g. `unsafe extern "C" printf(*const c_char, ...)` if using the `c_variadic` crate and nightly feature.

- Highly flexible API:
    - Customizable executable memory allocators via the [`JitAlloc`](https://docs.rs/closure-ffi/latest/closure_ffi/trait.JitAlloc.html) trait
    - `#![no_std]` support (though `alloc` is still required)
    - [untyped variants](https://docs.rs/closure-ffi/latest/closure_ffi/struct.UntypedBareFn.html) of `BareFn` types for when you need to store closures of different signatures in a collection
    - Traits that play nicely with type inference and that are powerful enough to write high-level abstraction around generic closures (e.g. the [hooking example](https://github.com/tremwil/closure-ffi/blob/master/examples/context_hooking.rs))
    - Customizable marker trait (`Send`/`Sync`) and lifetime bounds on the type-erased closure

- Optimized codegen: The crate leverages the compiler itself to monomorphize optimized bare function thunk templates for each function signature.
- Fast thunk generation: Most of the work is done at compile time, so the crate does not need to inspect argument types and manually emit instructions depending on the architecture and calling convention.
- Quasi zero-cost abstraction for function items and non-capturing closures: Since they are zero-sized types, the compiler-generated thunk template is *universally applicable* and no code needs to be emitted at runtime. For example, take the following code:
    ```rust
    extern "C" fn takes_fn(cb: unsafe extern "C" fn(u32) -> u32) { 
        // do something ...
    }

    extern "C" fn times_two(x: u32) -> u32 { 
        2 * x 
    }
    takes_fn(times_two);
    ```

    As of Rust 1.90, writing `takes_fn(|x| 2 * x)` does not compile since non-capturing closures can only coerce to `"Rust"` calling convention bare functions. Using closure-ffi in this situation is possible and essentially equivalent to the above: No memory is allocated and the few extra branches on the size of the closure will likely be optimized away.
    ```rust,ignore
    let bare_fn = closure_ffi::BareFn::new_c(|x: u32| 2 * x);
    takes_fn(bare_fn.bare());
    ```

# Examples
Passing a closure to a C API taking a contextless function pointer:

```rust
use closure_ffi::{BareFnMut};
// Imagine we have an foreign C API for registering and unregistering some callback function.
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


# Credits
- [tremwil](https://github.com/tremwil/): Library author and maintainer
- [Dasaav](https://github.com/Dasaav-dsv/): `lock (x14) push eax` x86 byte sequence idea