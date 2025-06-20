# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v3.0.0] - 2025-06-20

### Breaking Changes
- `ToBoxedUnsize` is now an unsafe trait.
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
  removing the need for the `bare_dyn` macro (which was not ideal as it would add an uncessary layer of
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
- Stop using .text relocations in asm thunks for compatiblity with platforms where they are not allowed
  (e.g. MacOS). Relocations are still used when `target_arch = "x86"`. (Fixes #5)

## [v0.4.0] - 2025-04-27

### Added
- CI checks
- Implementations of `JitAlloc` on `LazyLock` (and `spin::Lazy` on `no_std`) for easy use with statics

### Fixed
- ARM/Thumb support