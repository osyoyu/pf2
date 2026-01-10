Pf2
===========

A experimental sampling-based profiler for Ruby 3.3+.

- GitHub: https://github.com/osyoyu/pf2
- Documentation: https://osyoyu.github.io/pf2/


Notable Capabilites
--------

- Can accurately track multiple Ruby Threads' activity
- Sampling interval can be set based on per-Thread CPU usage
- Can record native (C-level) stack traces side-by-side with Ruby traces

Usage
--------

### Installation

You will need a C compiler to build the native extension.

Add this line to your application's Gemfile:

```ruby
gem 'pf2'

# When using the main branch, specify submodules: true
gem 'pf2', git: 'https://github.com/osyoyu/pf2.git', submodules: true
```

Pf2 can be installed as a standalone CLI tool as well.

```console
gem install pf2
```

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
$ pf2 report -o report.json my_program.pf2profile
```

Alternatively, `pf2 annotate` can be used to display hit counts side-by-side with source code.

```console
$ pf2 annotate my_program.pf2prof
```

### Configuration

Pf2 accepts the following configuration keys:

```rb
Pf2.start(
  interval_ms: 9,        # Integer: The sampling interval in milliseconds (default: 9)
  time_mode: :cpu,       # `:cpu` or `:wall`: The sampling timer's mode
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

### Scheduling

Schedulers determine when to execute sample collection, based on configuration (time mode and interval).

#### Signal-based scheduling

Pf2 schedules sample collection using POSIX timers. It creates a POSIX timer using `timer_create(3)` where available, or otherwise `setitimer(3)`. This leaves the actual time-keeping to the operating system kernel, which is capable of tracking accurate per-thread CPU time usage.

When the specified interval has arrived (the timer has _expired_), the OS delivers us a SIGPROF signal.

Signals are directed to Ruby Threads' underlying pthread, effectively "pausing" the Thread's activity. This routing is done using `SIGEV_THREAD_ID`, which is a Linux-only feature. Sample collection is done in the signal handler, which is expected to be more _accurate_, capturing the paused Thread's activity.

This scheduler heavily relies on Ruby's 1:N Thread model (1 Ruby Threads is strongly tied to a native pthread). It will not work properly in MaNy (`RUBY_MN_THREADS=1`).

#### ~~Timer-thread based scheduling~~

Note: Timer thread-based scheduling has been removed in v0.10.0, when the profiling backend has been rewritten in C. This may come back in the future if needed.

Another scheduler is the `TimerThreadScheduler`, which maintains a time-keeping thread by itself. A new native thread (pthread on Linux/macOS) will be created, and an infinite loop will be run inside. After `sleep(2)`-ing for the specified interval time, sampling will be queued using Ruby's Postponed Job API.

This scheduler is wall-time only, and does not support CPU-time based profiling.


Wishlist
--------

- [Flame Scopes](https://www.brendangregg.com/flamescope.html)
- more

Development
--------

See [doc/development.md](doc/development.md).


License
--------

The gem is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).
