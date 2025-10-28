# Changelog

## [v5.0.1] - 2025-10-28

### Fixed
- Use forked `iced-x86` crate to avoid conflicts with dependents using it with the `std` feature. This is temporary until a new iced version is released to allow `std` and `no_std` features to be enabled at the same time.

## [v5.0.0] - 2025-10-19

### Breaking Changes
- Compiling with a Thumb JSON target file will now require Nightly Rust.
- With the addition of the `safe_jit` feature, compiling with `--no-default-features` will now error
unless the `no_safe_jit` feature is explicitly enabled to prevent accidentally forgetting to enable `safe_jit`.
- Strengthened trait bounds on `FnPtr::CC` to make some APIs more ergonomic. This is technically a breaking change but is realistically harmless, as `FnPtr` should not be implemented by the end user. 
- Changed the `JitAlloc` blanket impl from `&J` for all `J: JitAlloc` to any type implementing `Deref<Target = J>`. This is more general and avoids having to write forwarding impls when putting a `JitAlloc` in a `LazyLock`, for example, but may break some downstream `JitAlloc` wrappers.

### Added
- Thunk generation is now fully safe thanks to the `safe_jit` feature, which uses a disassembler to properly relocate the prologue code instead of assuming it is trivially relocatable. This brings an end to this crate's UB issues.

- Support for the `efiapi` and `Rust` calling conventions.

### Fixed
- `global_jit_alloc` macro ambiguous parsing for the unsafe block variant.
- Incorrect relocation of thunk prologues on `i686-unknown-linux-gnu`.

### Removed
- i686-specific Windows calling conventions from x64 Windows targets.

## [v4.1.0] - 2025-09-23

### Changed
- Thunk generation modified to be a zero-cost abstraction: For functions items and non-capturing closures, constructing a `BareFn*` type will not allocate or emit code. Instead, it will use a compile-time template that conjures an instance of the ZST to invoke it.
- Added changelog to the documentation.
- Added UB warning to the documentation.

## [v4.0.0] - 2025-09-22

This update adds the scaffolding required to implement "higher order" transformations on bare function thunks. For example, it is now possible to write a function that synchronizes a generic `FnMutThunk` implementation while printing its return value:

```rust
use closure_ffi::{thunk_factory, cc, traits::{FnPtr, FnThunk, FnMutThunk}};

#[cfg(feature = "std")]
fn lock_and_debug<B: FnPtr, F: Send>(fun: F) -> impl FnThunk<B> + Sync
where
    for<'a, 'b, 'c> B::Ret<'a, 'b, 'c>: std::fmt::Debug,
    (cc::C, F): FnMutThunk<B>,
{
    let locked = std::sync::Mutex::new((cc::C, fun));
    thunk_factory::make_sync(move |args| unsafe {
        let ret = locked.lock().unwrap().call_mut(args);
        println!("value: {ret:?}");
        ret
    })
}
```

This is particularly useful for hooking libraries.

### Breaking Changes
- Removed `where Self: 'a + 'b + 'c` bounds on `FnPtr::Args` and `FnPtr::Ret`
- Regression in the expressivity of `bare_hrtb!()`: Now requires a `'static` bound on certain generic parameters
- removed zero-variant enum from `FnPtr::Args` for extern variaric functions to be able to implement the new trait functions. `FnPtr::call` now const panics instead of being impossible to call for them.

### Added
- `FnPtr::make_*_thunk` functions that can create a `Fn*Thunk` implementation from a closure with tuple-packed arguments.
- `FnOnceThunk::call_once`, `FnMutThunk::call_mut` and `FnThunk::call` for invoking the underlying closure with tuple-packed arguments.
- `thunk_factory` module for creating `Fn*Thunk` implementations that satisfy combinations of `Send` and `Sync` bounds.

### Fixed
- `libc` dependency not compatible with `no_std` on Linux ARM targets

## [v3.0.1] - 2025-06-21

### Fixed
- docs.rs build

## [v3.0.0] - 2025-06-20

### Breaking Changes
- `ToBoxedUnsize` has been renamed to `ToBoxedDyn` is now an unsafe trait. See the documentation for the
  new invariants.
- `Send` and `Sync` impl bounds on `BareFn` are now stricter to catch more unsafety.
- Major overhaul of feature flags. See README to view the changes.

### Added
- `UntypedBareFn*` types that erase the bare function type entirely. Can be used to store
  `BareFn*` wrappers of different types in a data structure.
- `coverage` unstable feature to support the `-C instrument-coverage` rustc flag.

### Changed
- Change thunk assembly magic numbers/sentinel values to sequences that are guaranteed to not be emitted by the compiler.
  Thanks to @Dasaav-dsv for the help.
- Move the arch/feature compile_error checks into the build script for better errors.
- Dual license under Apache-2.0 and MIT.

## [v2.4.0] - 2025-06-08

### Added
- `c_variadic` feature to add partial support for C variadic functions.

## [v2.3.0] - 2025-05-30

### Added
- `tuple_trait` feature to add a `core::marker::Tuple` bound to `FnPtr::Args`, allowing better
  interoperability with other Nightly features such as `fn_traits` and `unboxed_closures`.

### Changed
- use `dep:crate` optional dependency toggles to prevent implicit dependency named features.
  This is technically a breaking change, but as these features are not documented I have decided
  to not bump the major version.

## [v2.2.0] - 2025-05-30

### Fixed
- `bundled_jit_alloc` should now work on `i686-pc-windows-msvc` without linker errors

### Changed
- Bundled JIT allocator now uses [`jit-allocator2`](https://crates.io/crates/jit-allocator2), a 
  maintained fork of [`jit-allocator`](https://crates.io/crates/jit-allocator2) which fixes 
  a linker issue on `i686-pc-windows-msvc`.

## [v2.1.0] - 2025-05-29

### Added
- `from_ptr` and `to_ptr` methods to `FnPtr` trait, to avoid relying on `transmute_copy`
- `Send` and `Sync` supertraits on `FnPtr`
- Support for `C-unwind` and other `-unwind` calling conventions

## [v2.0.1] - 2025-05-29

### Fixed
- Typos in documentation

## [v2.0.0] - 2025-05-29

First stable release. `1.0.0` was skipped as significant changes to the API were made since the last
release.

### Breaking changes
- Changes to the trait system: bare function parameters now implement the `FnPtr` trait, which
  was carefully re-designed after attempting to build a function hooking library around `closure-ffi`.
  This required changes to the way higher-kinded bare functions are supported; see the doc for the new
  `bare_hrtb!` proc macro to learn more.

- Moved traits to the `traits` module. All traits used are now fully documented, including the `Fn*Thunk`
  traits used to generate the bare function thunks. This allows building a function-generic API that makes
  use of closure-ffi internally.

- Sweeping changes to the `BareFn*` generic parameters. The `BareFn*` types now type erase the closure,
  removing the need for the `bare_dyn` macro (which was not ideal as it would add an unnecessary layer of
  indirection). The DST used for type erasure is customizable via the type parameter `S` of `BareFn*Any`,
  with `BareFn*` and `BareFn*Send` now being type aliases for the common cases of no additional bounds
  and a `Send` bound on the closure, respectively.

- Removed the `bare_dyn!` macro as it is no longer needed now that `BareFn*` type-erases the closure.
- Replaced the `cc::hrtb!` by the `bare_hrtb!` macro, which now works differently. See the doc for more info.

### Added
- `unstable` feature enabling support for functionality locked behind unstable Rust features.

## [v0.5.0] - 2025-04-29

### Breaking Changes
- Changes to the `JitAlloc` API: `flush_instruction_cache` and `protect_jit_memory` now take `&self`
  as an argument. This was necessary to make `JitAlloc` dyn-compatible as part of the
  `custom_jit_alloc` feature.

### Added
- This changelog
- GH actions to automate publishing of the crate
- `custom_jit_alloc` feature allowing downstream crates to implement their own `GlobalJitAlloc`

### Fixed
- Stop using .text relocations in asm thunks for compatibility with platforms where they are not allowed
  (e.g. MacOS). Relocations are still used when `target_arch = "x86"`. (Fixes #5)

## [v0.4.0] - 2025-04-27

### Added
- CI checks
- Implementations of `JitAlloc` on `LazyLock` (and `spin::Lazy` on `no_std`) for easy use with statics

### Fixed
- ARM/Thumb support