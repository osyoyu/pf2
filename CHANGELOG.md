## [Unreleased]

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
