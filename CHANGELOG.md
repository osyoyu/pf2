## [Unreleased]

## [0.13.0] - 2026-01-18

### Added

- Pf2 should now have a dramatically lower memory footprint.
  - Samples are now stored in a compact hashmap internally.
  - See https://github.com/osyoyu/pf2/pull/85 for details.

### Fixed

- `pf2 serve` command now properly works. (Thanks @hanazuki)


## [0.12.0] - 2026-01-09

### Added

- `Pf2.profile` now accepts the same options as `Pf2.start`.
- The resulting profile now has `collected_sample_count` and `dropped_sample_count` fields.

### Fixed

- Samples captured after the collector thread was stopped now get included in the profile.
  - This shouldn't matter in practice (this all happens after `Pf2.stop` is called).

### Changed

- Accepted max stack depth is expanded to 1024 for Ruby (was 200) and 512 for native (was 300).
  - This is not configurable, but should be sufficient for most use cases. Please open an issue if you need higher limits.
- Pf2.profile now accepts the same parameters as Pf2.start.
- Internal changes
  - Updated libbacktrace to the latest version as of 2026/1/8.
  - Tests are now much more stabilized.


## [0.11.3] - 2025-12-28

This version is for testing the new release process through [Trusted Publishing](https://guides.rubygems.org/trusted-publishing/). All code is identical to 0.11.2.


## [0.11.2] - 2025-12-28

0.11.1 was yanked since it was accidentally published without libbacktrace vendored. Use 0.11.2.

### Fixed

- Fixed issues preventing builds on macOS.


## [0.11.0] - 2025-12-27

### Added

- RDoc documentation is now online - https://osyoyu.github.io/pf2/
- Native stack consolidation now supports LTO-ed binaries (@hanazuki)

### Changed

- `Pf2c` module is now completely removed. `Pf2c::Session` has been merged as `Pf2::Session`.

### Fixed

- Fixed an bug where the program crashes when a `Pf2::Session` is GC'd before profiling starts.
- Fixed an bug where the program crashes when the native stack was more than 200 frames deep.


## [0.10.0] - 2025-12-26

### Added

**This version contains a complete rewrite of the profiler!**

- The default sample collection backend has been switched to the new C-based backend.
  - The previous Rust-based backed has been removed. Use v0.9.0 if you need it.
- macOS / non-Linux platform support!
  - On platforms which lack `timer_create(3)` such as macOS, Pf2 now fall backs to `setitimer(3)` based sampling. This mode does not support per-thread CPU time sampling.

### Changed

- `logger` is now declared as a dependency (Ruby 4.0 compat).


## [0.9.0] - 2025-03-22

## Added

- `pf2 annotate` command
- A new sample collection backend implemented in C

## Changed

- Set SA_RESTART flag to reduce EINTRs in profiled code


## [0.8.0] - 2025-01-27

## Added

- The new serializer (Ser2) is now available in `Pf2::Session#start` through the `use_experimental_serializer` option.
  - This serializer is more efficient and has a smaller memory footprint than the default serializer.
  - Ser2 still lacks some features, such as weaving of native stacks.


## [0.7.1] - 2025-01-02

### Fixed

- Reverted Cargo.lock version to 3 to support older versions of Rust (<1.78).


## [0.7.0] - 2025-01-03

### Changed

- Prepended `frozen_string_literal: true` to all Ruby files.
- Internals
  - Updated rb-sys to 0.9.105.
  - Synced libbacktrace with upstream as of 2024-08-06.


## [0.6.0] - 2024-07-15

### Changed

- The default sampling interval is now 9 ms (was originally 49 ms).
  - It is intentional that the default is not 10 (or 50) ms - this is to avoid lockstep sampling.
- BREAKING: `Pf2::Reporter` has been moved to `Pf2::Reporter::FirefoxProfiler`.
  - This is to make space for other planned reporters.


## [0.5.2] - 2024-07-13

### Fixed

- Properly default to TimerThread scheduler on non-Linux environments.


## [0.5.1] - 2024-03-25

### Fixed

- Fixed compilation on non-Linux environments.


## [0.5.0] - 2024-03-25

### Added

- `pf2 serve` subcommand
  - `pf2 serve -- ruby target.rb`
  - Profile programs without any change
- New option: `threads: :all`
  - When specified, Pf2 will track all active threads.
  - `threads: nil` / omitting the `threads` option has the same effect.
- Introduce `Pf2::Session` (https://github.com/osyoyu/pf2/pull/16)
  - `Session` will be responsible for managing Profiles and Schedulers

### Removed

- `Pf2::SignalScheduler` and `Pf2::TimerThreadScheduler` are now hidden from Ruby.
- `track_all_threads` option is removed in favor of `threads: :all`.


## [0.4.0] - 2024-03-22

### Added

- New option: `track_all_threads`
  - When true, all Threads will be tracked regardless of the `threads` option.

### Removed

- The `track_new_threads` option was removed in favor of the `track_all_threads` option.


## [0.3.0] - 2024-02-05

### Added

- Native stack consolidation
  - Pf2 now records native (C-level) stacks during sample capture.
    - This functionality is based on [libbacktrace](https://github.com/ianlancetaylor/libbacktrace).
- New configuration interface for `Pf2.start`, `Pf2::SignalScheduler.start`, `Pf2::TimerThreadScheduler.start`
  - They now accept keyword arguments (`interval_ms`, `threads`, `time_mode`, `track_new_threads`).
- New configuration options
  - `interval_ms`: The sampling interval.
  - `time_mode` (`:wall` or `:cpu`): The sampling timer's _mode_. `:wall` is wall-clock time (CLOCK_MONOTONIC to be specific), `:cpu` is per-thread CPU time (CLOCK_THREAD_CPUTIME_ID).

### Removed

- Configuration through positional arguments is no longer supported.


## [0.2.0] - 2024-01-21

- New Ruby interface: Pf2.start, Pf2.stop, Pf2.profile
- Introduce the concepts of Schedulers
  - Implement SignalScheduler and TimerThreadScheduler
- Rewritten many components


## [0.1.0] - 2023-10-04

- Initial release
