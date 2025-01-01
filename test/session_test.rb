# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'

class SessionTest < Minitest::Test
  def test_default_options
    config = Pf2::Session.new.configuration
    assert_equal(:signal, config[:scheduler])
    assert_equal(9, config[:interval_ms])
    assert_equal(:cpu, config[:time_mode])
  end

  def test_scheduler_option
    config = Pf2::Session.new(scheduler: :timer_thread, time_mode: :wall, threads: []).configuration
    assert_equal(:timer_thread, config[:scheduler])
  end

  def test_interval_ms_option
    config = Pf2::Session.new(interval_ms: 1, threads: []).configuration
    assert_equal(1, config[:interval_ms])
  end

  def test_time_mode_option
    config = Pf2::Session.new(time_mode: :wall, threads: []).configuration
    assert_equal(:wall, config[:time_mode])
  end

  def test_timer_thread_scheduler_does_not_accept_cpu_time_mode
    assert_raises(ArgumentError) do
      config = Pf2::Session.new(scheduler: :timer_thread, time_mode: :cpu, threads: [])
    end
  end
end
