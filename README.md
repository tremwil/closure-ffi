Provides wrappers around closures which allows them to be called through context-free unsafe
bare functions. For example:

Context-free bare functions are not needed very often, as properly designed C APIs typically
allow the user to specify an opaque pointer to a context object which will be provided to the
function pointer. However, this is not always the case, and may be impossible in less common
scenarios, e.g. function hooking for game modding/hacking.

### Example

```rust
use closure_ffi::BareFnMut;
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

# Supported Configurations

## Targets

closure-ffi currently supports the following platforms (with some features being restricted to others). `no_std` is supported, but a [global allocator](https://doc.rust-lang.org/stable/alloc/alloc/trait.GlobalAlloc.html) must be registered with the `alloc` crate.

| Target | `safe_jit` | `default_jit_alloc` | CI tested? |
|-|-|-|-|
| `x86_64` (unix) | ✅ | ✅ | ✅ (linux) | 
| `x86_64` (windows) | ✅ | ✅ | ✅ (msvc) |
| `x86_64` (none) | ✅ | ❌ | ❌ |
| `i686` (unix) | required (*) | ✅ | ❌ |
| `i686` (windows) | ✅ | ✅ | ✅ (msvc) |
| `i686` (none) | ✅ | ❌ | ❌ |
| `aarch64-apple-darwin` | ✅ (**) | ✅ | ✅ |
| `aarch64` (unix) | ✅ (**) | ✅ | ✅ (linux) | 
| `aarch64` (windows) | ✅ (**) | ✅ | ❌ |
| `aarch64` (none) | ✅ (**) | ❌ | ❌ |
| `arm` (linux) | ✅ (**) | ✅ | ❌ |
| `arm` (none) | ✅ (**) | ❌ | ❌ |
| `thumbv7` (linux) | ✅ (**) | ✅ | ❌ |
| `thumbv7` (none) | ✅ (**) | ❌ | ❌ |

*(\*)*: The feature is required as disabling it would lead to incorrect code being emitted.

*(\*\*)*: Depends on Capstone, which requires a C toolchain and some libc headers to build.

## Executable Memory Allocators

closure-ffi abstracts away executable memory allocators via the [`JitAlloc`](https://docs.rs/closure-ffi/latest/closure_ffi/trait.JitAlloc.html) trait. `BareFn` types can be constructed using an arbitrary `JitAlloc` implementation using the constructor methods with a `_in` suffix.

The `default_jit_alloc` feature (enabled by default) provides the built-in global `JitAlloc` implementation. You can override the global implementation by disabling it and enabling `global_jit_alloc` instead. 

## Calling conventions

The following calling conventions (and all `-unwind` variants) are supported. Calling convention marker types can be found in the `cc` module.

- All "standard" foreign calling conventions like `C`, `system` and `efiapi`.
- The `Rust` calling convention. Note that **this calling convention is unstable and ABI compatibility is only guaranteed within a particular binary!**
- On x64 Windows, the `win64` calling convention.
- On x86 Windows, the `stdcall`, `cdecl`, `fastcall` and `thiscall` calling conventions.
- On non-Windows x64, the `sysv64` calling convention.
- On ARM (not Aarch64), the `aapcs` calling convention.

## Signatures

The following function signatures are supported:

- Functions of up to 12 arguments with arbitrary argument types. This means that *all* ffi-safe types can be used in the function signature: thin references, `#[repr(C)]` types, `Option<&T>`, `NonNull`, [thin `CStr`](https://crates.io/crates/thin_cstr) refs, etc. Note that you will **not** get a warning if using a non ffi-safe type in the function signature.

- Lifetime-generic (a.k.a. higher-kinded) bare functions, e.g. `for<'a, 'b> unsafe extern "C" fn(&'a CStr, &'b CStr) -> &'a CStr` through the `bare_hrtb!` macro (requires the `proc_macros` feature).

- Variadic C functions e.g. `unsafe extern "C" printf(*const c_char, ...)` are supported when the `c_variadic` crate and nightly feature are enabled.

## Features Flags

The crate comes with the following feature flags:

### Stable
- `std` (**default**): Use `std` features. When this is turned off, the crate is compatible with `no_std`,
  although a global allocator must be defined.

- `global_jit_alloc` (**default**): Provides the `GlobalJitAlloc` ZST which defers to a global JIT allocator implementation provided either through `default_jit_alloc` feature or the `global_jit_alloc!` macro. This is necessary to construct `BareFn` types without explicitly passing an allocator.

- `default_jit_alloc` (**default**): Provides a global JIT allocator implementation through the 
  [`jit-allocator2`](https://crates.io/crates/jit-allocator2) crate. Note that said crate relies on operating system APIs, so not all configurations are supported. See the [Targets](#targets) section for details.

- `proc_macros`: Provides the `bare_hrtb` proc macro which is necessary for creating bare
  functions with signatures that involve higher-kinded lifetimes (i.e. `for<'a, ...>` statements).

- `safe_jit` (**default**): Implements disassembler-aided relocation of the thunk template prologue. This is not so much a feature as it is an integral part of the crate.  

  Without it, the crate makes the (unsafe) assumption that the thunk prologues are trivially relocatable, and blocks certain compiler optimizations to try to uphold this. However, **this is not guaranteed and UB is a real possibility**. While this feature can be disabled to improve compatibility with targets for which the dependency on the Capstone disassembler (a C library) cannot be built, I would strongly suggest not doing so.

- `no_safe_jit`: Since not having `safe_jit` enabled is inherently unsafe, the crate will refuse to build unless this feature is enabled to prevent accidentally forgetting `safe_jit` on `--no-default-feature` builds.

### Unstable (require a nightly compiler)
- `unstable`: Enable the use of unstable Rust features for aspects of the crate that benefit from 
  them without causing any API breaks. Unstable features that can cause breaking changes when enabled 
  are gated separately, and also enable this feature.
- `tuple_trait`: Adds a [`core::marker::Tuple`](https://doc.rust-lang.org/nightly/core/marker/trait.Tuple.html)
  bound on `FnPtr::Args`. This allows downstream crates to easily integrate the library with closure-related
  nightly features such as `unboxed_closures` and `fn_traits`.
- `c_variadic`: Adds *partial* (no invocation through `call`) `FnPtr` and `Fn*Thunk` implementations for variadic functions.
- `coverage`: Enables support for the `-C instrument-coverage` compiler flag.

# How it Works

Unlike [libffi](https://github.com/libffi/libffi) and similar libraries, this crate leverages the Rust compiler itself to monomorphize optimized bare function *thunk templates* for each function signature. For example, given `F: Fn(usize) -> usize`, a thunk template for the "C" calling convention on `x86_64` would look like this:
```rust,ignore
unsafe extern "C" fn thunk(arg0: usize) -> usize {
    let closure_ptr: *const F;

    core::arch::asm!(
        "mov {cl_addr}, [rip + 2f]",
        "jmp [rip + 2f+$8]",
        ".balign 8, 0xCC",
        "2:",
        ".8byte {cl_magic_0}", // closure pointer
        ".8byte {cl_magic_1}", // thunk exit addrsss
        cl_magic_0 = const { CL_MAGIC[0] },
        cl_magic_1 = const { CL_MAGIC[1] },
        cl_addr = out(reg) $closure_ptr,
        options(nostack)
    );

    (&*closure_ptr)(arg0)
}
```

where `CL_MAGIC` is a sequence of invalid or reserved undefined (UDF) instructions that will not be found in a compiler-generated function prologue.

When instantiated for a particular instance of the closure, the magic constant is searched to find where to write a pointer to it as well as the address of the next instruction past the `asm!` block. A disassembler is then used to relocate the code up to the end of the `asm!` block to dynamically allocated executable memory.

This is very fast at runtime, since most work is done at compile time and the crate does not need to inspect argument types and manually emit instructions depending on the architecture and calling convention. The compiler can also inline the closure's code into the thunk template, optimizing the prologue and avoiding further branches or stack spilling.

## Non-capturing closures

If the `Fn` impl is a zero-sized type, such as a non-capturing closure or a function item, it is possible to "conjure" a valid reference to the type from a dangling pointer. Hence a thunk template like this is valid for all instances of the closure:
```rust,ignore
unsafe extern "C" fn thunk(arg0: usize) -> usize {
    let closure_ptr: *const F = core::ptr::dangling();
    (&*closure_ptr)(arg0)
}
```

This optimization lets closure-ffi thunk this kind of closure without allocating or emitting any code at runtime, making `BareFn` a quasi zero-cost abstraction. For example, consider the following code:
```rust,ignore
extern "C" fn takes_fn(cb: unsafe extern "C" fn(u32) -> u32) { 
    // do something ...
}

extern "C" fn times_two(x: u32) -> u32 { 
    2 * x 
}
takes_fn(times_two);
```

Using closure-ffi in this situation is possible and essentially equivalent to the above: No memory is allocated and the few extra branches on the size of the closure will likely be optimized away:
```rust,ignore
let bare_fn = closure_ffi::BareFn::new_c(|x: u32| 2 * x);
takes_fn(bare_fn.bare());
```

# Credits
- [tremwil](https://github.com/tremwil/): Library author and maintainer
- [Dasaav](https://github.com/Dasaav-dsv/): `lock (x14) push eax` x86 magic byte sequence idea