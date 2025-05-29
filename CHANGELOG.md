# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2025-05-29

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

## [0.5.0] - 2025-04-29

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

## [0.4.0] - 2025-04-27

### Added
- CI checks
- Implementations of `JitAlloc` on `LazyLock` (and `spin::Lazy` on `no_std`) for easy use with statics

### Fixed
- ARM/Thumb support