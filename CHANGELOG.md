# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2025-04-28

### Added
- This changelog
- GH actions to automate publishing of the crate

### Fixed
- Stop using .text relocations in asm thunks for compatiblity with platforms where they are not allowed (e.g. MacOS)

## [0.4.0] - 2025-04-27

### Added
- CI checks
- Implementations of `JitAlloc` on `LazyLock` (and `spin::Lazy` on `no_std`) for easy use with statics

### Fixed
- ARM/Thumb support