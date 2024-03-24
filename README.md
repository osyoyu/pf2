Pf2
===========

A experimental sampling-based profiler for Ruby 3.3+.

Notable Capabilites
--------

- Can accurately track multiple Ruby Threads' activity
- Sampling interval can be set based on per-Thread CPU usage
- Can record native (C-level) stack traces side-by-side with Ruby traces

Usage
--------

### Quickstart

Run your Ruby program through `pf2 serve`.
Wait a while until Pf2 collects profiles (or until the target program exits), then open the displayed link for visualization.

```
$ pf2 serve -- ruby target.rb
[Pf2] Listening on localhost:51502.
[Pf2] Open https://profiler.firefox.com/from-url/http%3A%2F%2Flocalhost%3A51502%2Fprofile for visualization.

I'm the target program!
```

### Profiling

Pf2 will collect samples every 10 ms of wall time by default.

```ruby
# Threads in `threads` will be tracked
Pf2.start(threads: [Thread.current])

your_code_here

# Stop profiling and save the profile for visualization
profile = Pf2.stop
File.write("my_program.pf2profile", profile)
```

Alternatively, you may provide a code block to profile.

```ruby
profile = Pf2.profile do
  your_code_here() # will be profiled
  Thread.new { threaded_code() } # will also be profiled
end

# Save the profile for visualization
File.write("my_program.pf2profile", profile)
```

### Reporting / Visualization

Profiles can be visualized using the [Firefox Profiler](https://profiler.firefox.com/).

```console
$ pf2 -o report.json my_program.pf2profile
```

### Configuration

Pf2 accepts the following configuration keys:

```rb
Pf2.start(
  interval_ms: 49,        # Integer: The sampling interval in milliseconds (default: 49)
  time_mode: :cpu,        # `:cpu` or `:wall`: The sampling timer's mode
                          # (default: `:cpu` for SignalScheduler, `:wall` for TimerThreadScheduler)
  threads: [th1, th2],    # `Array<Thread>` | `:all`: A list of Ruby Threads to be tracked.
                          # When `:all` or unspecified, Pf2 will track all active Threads.
)
```


Overhead
--------

While Pf2 aims to be non-disturbulent as much as possible, a small overhead still is incured.

(TBD)

Limitations
--------

Pf2 cannot properly track program activity in some known cases. I'm working to remove these limtations, so stay tuned.

- Program execution in forked processes
  - Workarounds available for Puma
- Program execution in Fibers
- Program execution when MaNy (`RUBY_MN_THREADS`) is enabled

Internals
--------

### Sampling

Pf2 is a _sampling profiler_. This means that Pf2 collects _samples_ of program execution periodically, instead of tracing every action (e.g. method invocations and returns).

Pf2 uses the `rb_profile_thread_frames()` API for sampling. When to do so is controlled by _Schedulers_, described in the following section.

### Schedulers

Schedulers determine when to execute sample collection, based on configuration (time mode and interval). Pf2 has two schedulers available.

#### SignalScheduler (Linux-only)

The first is the `SignalScheduler`, based on POSIX timers. Pf2 will use this scheduler when possible. SignalScheduler creates a POSIX timer for each Ruby Thread (the underlying pthread to be more accurate) using `timer_create(3)`. This leaves the actual time-keeping to the OS, which is capable of tracking accurate per-thread CPU time usage.

When the specified interval has arrived (the timer has _expired_), the OS delivers us a SIGALRM (note: Unlike `setitimer(2)`, `timer_create(3)` allows us to choose which signal to be delivered, and Pf2 uses SIGALRM regardless of time mode). This is why the scheduler is named SignalScheduler.

Signals are directed to Ruby Threads' underlying pthread, effectively "pausing" the Thread's activity. This routing is done using `SIGEV_THREAD_ID`, which is a Linux-only feature. Sample collection is done in the signal handler, which is expected to be more _accurate_, capturing the paused Thread's activity.

This scheduler heavily relies on Ruby's 1:N Thread model (1 Ruby Threads is strongly tied to a native pthread). It will not work properly in MaNy (`RUBY_MN_THREADS=1`).

#### TimerThreadScheduler

Another scheduler is the `TimerThreadScheduler`, which maintains a time-keeping thread by itself. A new native thread (pthread on Linux/macOS) will be created, and an infinite loop will be run inside. After `sleep(2)`-ing for the specified interval time, sampling will be queued using Ruby's Postponed Job API.

This scheduler is wall-time only, and does not support CPU-time based profiling.

Future Plans
--------

- Remove known limitations, if possible
- Implement a "tracing" scheduler, using the C TracePoint API
- more


License
--------

The gem is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).
