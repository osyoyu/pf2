# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'

class SessionTest < Minitest::Test
  def test_default_options
    session = Pf2::Session.new

    assert_equal(9, session.configuration[:interval_ms])
    assert_equal(:cpu, session.configuration[:time_mode])
    assert_equal(200, session.configuration[:max_depth])
    assert_equal(300, session.configuration[:max_native_depth])
  end

  def test_interval_ms_option
    session = Pf2::Session.new(interval_ms: 1)
    assert_equal(1, session.configuration[:interval_ms])
  end

  def test_time_mode_option
    session = Pf2::Session.new(time_mode: :wall)
    assert_equal(:wall, session.configuration[:time_mode])
  end

  def test_depth_options
    session = Pf2::Session.new(max_depth: 700, max_native_depth: 123)
    assert_equal(700, session.configuration[:max_depth])
    assert_equal(123, session.configuration[:max_native_depth])
  end

  def test_depth_options_must_be_positive
    Pf2::Session.new(max_depth: 1, max_native_depth: 1)
    assert_raises(ArgumentError) { Pf2::Session.new(max_depth: 0) }
    assert_raises(ArgumentError) { Pf2::Session.new(max_native_depth: 0) }
  end

  def test_depth_options_must_not_exceed_limit
    Pf2::Session.new(max_depth: 8192, max_native_depth: 8192)
    assert_raises(ArgumentError) { Pf2::Session.new(max_depth: 8193) }
    assert_raises(ArgumentError) { Pf2::Session.new(max_native_depth: 8193) }
  end
end
