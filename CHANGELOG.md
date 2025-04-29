# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
  (e.g. MacOS). relocations are still used when `target_arch = "x86"`. (Fixes #5)

## [0.4.0] - 2025-04-27

### Added
- CI checks
- Implementations of `JitAlloc` on `LazyLock` (and `spin::Lazy` on `no_std`) for easy use with statics

### Fixed
- ARM/Thumb support