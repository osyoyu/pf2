Pf2
===========

A experimental sampling-based profiler for Ruby 3.3+.

Notable Capabilites
--------

- Can accurately track multiple Ruby Threads' activity
- Sampling interval can be set based on per-Thread CPU usage

Usage
--------

### Profiling

Pf2 will collect samples every 10 ms of wall time by default.

```ruby
profiler = Pf2::Profiler.new([Thread.current])
profiler.start

# your code goes here

profile = profiler.stop
File.write("my_program.pf2profile", profile)
```

### Reporting / Visualization

Profiles can be visualized using the [Firefox Profiler](https://profiler.firefox.com/).

```console
$ pf2 -o report.json my_program.pf2profile
```

### Configuration

(TBD)

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

(TBD)


Future Plans
--------

- Remove known limitations, if possible
- Implement a "tracing" scheduler, using the C TracePoint API
- more


License
--------

The gem is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).
