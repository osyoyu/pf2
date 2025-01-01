# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'
require 'pf2/reporter'

class StackWeaverTest < Minitest::Test
  def test_ruby_only
    profile = {
      start_timestamp_ns: 1,
      duration_ns: 1,
      samples: [
        { stack: [1, 0, 0], native_stack: [], ruby_thread_id: 1 }
      ],
      locations: [
        { function_index: 0, lineno: 1, address: nil },
        { function_index: 1, lineno: 1, address: nil },
      ],
      functions: [
        { implementation: :ruby, name: "one", filename: nil, start_lineno: 1, start_address: nil },
        { implementation: :ruby, name: "two", filename: nil, start_lineno: 2, start_address: nil },
      ],
    }
    stack_weaver = Pf2::Reporter::StackWeaver.new(profile)

    assert_equal(
      [0, 0, 1].reverse,
      stack_weaver.weave(profile[:samples][0][:stack], profile[:samples][0][:native_stack])
    )
  end

  def test_both
    profile = {
      start_timestamp_ns: 1,
      duration_ns: 1,
      samples: [
        { stack: [1, 0], native_stack: [3, 2], ruby_thread_id: 1 }
      ],
      locations: [
        { function_index: 0, lineno: 1, address: nil },
        { function_index: 1, lineno: 1, address: nil },
        { function_index: 2, lineno: 1, address: nil },
        { function_index: 3, lineno: 1, address: nil },
      ],
      functions: [
        { implementation: :ruby, name: "<main>", filename: nil, start_lineno: 1, start_address: nil },
        { implementation: :ruby, name: "pure_ruby", filename: nil, start_lineno: 2, start_address: 123 },
        { implementation: :native, name: "main", filename: nil, start_lineno: 1, start_address: nil },
        { implementation: :native, name: "vm_exec_core", filename: nil, start_lineno: 1, start_address: nil },
      ],
    }
    stack_weaver = Pf2::Reporter::StackWeaver.new(profile)

    assert_equal(
      [2, 3, 0, 1].reverse,
      stack_weaver.weave(profile[:samples][0][:stack], profile[:samples][0][:native_stack])
    )
  end

  def test_cfunc
    profile = {
      start_timestamp_ns: 1,
      duration_ns: 1,
      samples: [
        { stack: [1, 0], native_stack: [4, 3, 2], ruby_thread_id: 1 }
      ],
      locations: [
        { function_index: 0, lineno: 1, address: nil },
        { function_index: 1, lineno: 1, address: nil },
        { function_index: 2, lineno: 1, address: nil },
        { function_index: 3, lineno: 1, address: nil },
        { function_index: 4, lineno: 1, address: nil },
      ],
      functions: [
        { implementation: :ruby, name: "<main>", filename: nil, start_lineno: 1, start_address: nil },
        { implementation: :ruby, name: "cfunc", filename: nil, start_lineno: 2, start_address: 123 },
        { implementation: :native, name: "main", filename: nil, start_lineno: 1, start_address: nil },
        { implementation: :native, name: "vm_exec_core", filename: nil, start_lineno: 1, start_address: nil },
        { implementation: :native, name: "native", filename: nil, start_lineno: 1, start_address: 123 },
      ],
    }
    stack_weaver = Pf2::Reporter::StackWeaver.new(profile)

    assert_equal(
      [2, 3, 0, 1, 4].reverse,
      stack_weaver.weave(profile[:samples][0][:stack], profile[:samples][0][:native_stack])
    )
  end
end
