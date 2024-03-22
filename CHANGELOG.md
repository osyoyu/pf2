## [Unreleased]


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
