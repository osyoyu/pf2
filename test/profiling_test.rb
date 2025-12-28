# frozen_string_literal: true

require 'minitest/autorun'
require_relative './test_helper.rb'

require 'pf2'

def loop_addition
  1000000.times do
    (+"foo") + (+"bar")
  end
end

class ProfilingTest < Minitest::Test
  def test_profiling_loop_addition
    session = Pf2::Session.new(interval_ms: 1)
    session.start
    start_time = Process.clock_gettime(Process::CLOCK_PROCESS_CPUTIME_ID)
    loop_addition
    end_time = Process.clock_gettime(Process::CLOCK_PROCESS_CPUTIME_ID)
    profile = session.stop

    elapsed_time = end_time - start_time
    expected_samples = (elapsed_time * 1000).to_i # 1000 Hz sampling rate
    acceptable_hi = (expected_samples * 1.2).to_i
    acceptable_lo =
      if RUBY_PLATFORM.include?("darwin")
        (expected_samples * 0.3).to_i # macOS has coarser timer?
      else
        (expected_samples * 0.8).to_i
      end

    assert_operator profile[:samples].size, :>=, acceptable_lo, "Expected at least #{acceptable_lo} samples (runtime: #{sprintf("%.4f", elapsed_time)} s)"
    assert_operator profile[:samples].size, :<, acceptable_hi, "Expected no more than #{acceptable_hi} samples (runtime: #{sprintf("%.4f", elapsed_time)} s)"
  end

  def test_capture_thread_id
    session = Pf2::Session.new
    session.start
    loop_addition
    profile = session.stop

    assert_instance_of Integer, profile[:samples][0][:ruby_thread_id]
  end

  def test_multiple_threads
    session = Pf2::Session.new
    session.start
    th1 = Thread.new { loop_addition }
    th2 = Thread.new { loop_addition }
    th1.join
    th2.join
    profile = session.stop

    assert_equal 2, profile[:samples].map { |s| s[:ruby_thread_id] }.uniq.size
  end
end
