# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'

class SessionTest < Minitest::Test
  def test_default_options
    session = Pf2::Session.new

    assert_equal(9, session.configuration[:interval_ms])
    assert_equal(:cpu, session.configuration[:time_mode])
  end

  def test_interval_ms_option
    session = Pf2::Session.new(interval_ms: 1)
    assert_equal(1, session.configuration[:interval_ms])
  end

  def test_time_mode_option
    session = Pf2::Session.new(time_mode: :wall)
    assert_equal(:wall, session.configuration[:time_mode])
  end
end
