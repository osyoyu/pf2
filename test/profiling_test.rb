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
end
