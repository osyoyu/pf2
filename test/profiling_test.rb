# frozen_string_literal: true

require 'minitest/autorun'
require_relative './test_helper.rb'

require 'pf2'

class ProfilingTest < Minitest::Test
  def test_single_capture
    session = Pf2::Session.new(_test_no_install_timer: true)
    session.start
    sample_now
    profile = session.stop

    assert_equal 1, profile[:samples].size
  end

  def test_capture_thread_id
    session = Pf2::Session.new
    session.start
    sample_now
    profile = session.stop

    assert_instance_of Integer, profile[:samples][0][:ruby_thread_id]
  end

  def test_multiple_threads
    skip 'This test is actually broken; Pf2c fails to target threads'

    session = Pf2::Session.new(_test_no_install_timer: true)
    session.start
    th1 = Thread.new { loop_addition }
    th2 = Thread.new { loop_addition }
    th1.join
    th2.join
    profile = session.stop

    assert_equal 2, profile[:samples].map { |s| s[:ruby_thread_id] }.uniq.size
  end

  def test_collected_sample_count
    session = Pf2::Session.new(_test_no_install_timer: true)
    session.start
    10.times do
      sample_now
      # Allow some time for the signal to be handled
      # (otherwise signals may be collapsed)
      sleep 0.01
    end
    profile = session.stop

    assert_equal 10, profile[:collected_sample_count]
  end

  def test_dropped_sample_count
    session = Pf2::Session.new(_test_no_install_timer: true)
    previous_stress = GC.stress

    session.start
    begin
      GC.stress = true # trigger sample dropping due to GC
      50.times { Object.new; +"foo" + +"bar" }
    ensure
      GC.stress = previous_stress
    end
    profile = session.stop

    assert_kind_of(Integer, profile[:dropped_sample_count])
    assert_operator(profile[:dropped_sample_count], :>=, 0)
  end
end
