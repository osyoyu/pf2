# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'

class Pf2Test < Minitest::Test
  def test_profile_returns_profile
    profile = Pf2.profile do
      100.times { 1 + 1 }
    end

    assert_instance_of Hash, profile
    assert profile.key?(:start_timestamp_ns)
    assert profile.key?(:duration_ns)
    assert profile.key?(:samples)
    assert profile.key?(:locations)
    assert profile.key?(:functions)
  end

  def test_profile_default_options
    Pf2.profile do
      session = Pf2.class_variable_get(:@@session)

      assert_equal 9, session.configuration[:interval_ms]
      assert_equal :cpu, session.configuration[:time_mode]
    end
  end

  def test_profile_passes_custom_options_to_start
    Pf2.profile(interval_ms: 123, time_mode: :wall) do
      session = Pf2.class_variable_get(:@@session)

      assert_equal 123, session.configuration[:interval_ms]
      assert_equal :wall, session.configuration[:time_mode]
    end
  end

  def test_profile_raises_error_when_called_without_block
    assert_raises(ArgumentError, "block required") { Pf2.profile }
  end

  def test_profile_stops_session_even_if_given_block_raises
    begin
      Pf2.profile do
        raise
      end
    rescue
      # nice catch, do nothing
    end

    assert_nil Pf2.class_variable_get(:@@session)
  end
end
