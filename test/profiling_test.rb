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
    return if !defined?(Pf2c)

    session = Pf2c::Session.new
    session.start
    loop_addition
    profile = session.stop

    assert_operator profile[:samples].size, :>=, 10, "Expected at least 10 samples"
    assert_operator profile[:samples].size, :<, 30, "Expected no more than 30 samples"
  end

  def test_capture_thread_id
    return if !defined?(Pf2c)

    session = Pf2c::Session.new
    session.start
    loop_addition
    profile = session.stop

    assert_instance_of Integer, profile[:samples][0][:ruby_thread_id]
  end

  def test_multiple_threads
    return if !defined?(Pf2c)

    session = Pf2c::Session.new
    session.start
    th1 = Thread.new { loop_addition }
    th2 = Thread.new { loop_addition }
    th1.join
    th2.join
    profile = session.stop

    assert_equal 2, profile[:samples].map { |s| s[:ruby_thread_id] }.uniq.size
  end
end
