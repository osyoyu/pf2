## [Unreleased]

## [0.11.2] - 2025-12-28

0.11.1 was accidentally published without libbacktrace vendored.

## [0.11.1] - 2025-12-28

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
