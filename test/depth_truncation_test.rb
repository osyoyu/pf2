# frozen_string_literal: true

require 'minitest/autorun'
require_relative './test_helper.rb'

require 'pf2'

def deep_call(level)
  return 1000.times { 1 + 1 } if level <= 0
  deep_call(level - 1)
end

class DepthTruncationTest < Minitest::Test
  def test_stacks_are_truncated_to_configured_depths
    profile = Pf2.profile(interval_ms: 1, max_depth: 5) do
      deadline = Process.clock_gettime(Process::CLOCK_PROCESS_CPUTIME_ID) + 0.05
      while Process.clock_gettime(Process::CLOCK_PROCESS_CPUTIME_ID) < deadline
        deep_call(10)
      end
    end

    assert_not_empty profile[:samples], "expected profiler to collect samples"

    profile[:samples].each do |sample|
      assert_operator sample[:stack].length, :<=, 5
      assert_operator sample[:native_stack].length, :<=, 3
    end
  end
end
