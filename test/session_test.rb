# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'

class SessionTest < Minitest::Test
  def test_default_options
    session = if defined?(Pf2c)
      Pf2c::Session.new
    else
      Pf2::Session.new
    end

    assert_equal(:signal, session.configuration[:scheduler]) if !defined?(Pf2c)
    assert_equal(9, session.configuration[:interval_ms])
    assert_equal(:cpu, session.configuration[:time_mode])
  end

  def test_scheduler_option
    return if defined?(Pf2c) # not implemented in Pf2c

    session = Pf2::Session.new(scheduler: :timer_thread, time_mode: :wall, threads: [])
    assert_equal(:timer_thread, session.configuration[:scheduler])
  end

  def test_interval_ms_option
    session = if defined?(Pf2c)
      Pf2c::Session.new(interval_ms: 1)
    else
      Pf2::Session.new(interval_ms: 1, threads: [])
    end
    assert_equal(1, session.configuration[:interval_ms])
  end

  def test_time_mode_option
    session = if defined?(Pf2c)
      Pf2c::Session.new(time_mode: :wall)
    else
      Pf2::Session.new(time_mode: :wall, threads: [])
    end
    assert_equal(:wall, session.configuration[:time_mode])
  end

  def test_timer_thread_scheduler_does_not_accept_cpu_time_mode
    return if defined?(Pf2c) # not implemented in Pf2c

    assert_raises(ArgumentError) do
      Pf2::Session.new(scheduler: :timer_thread, time_mode: :cpu, threads: [])
    end
  end
end
