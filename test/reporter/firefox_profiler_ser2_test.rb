# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'
require 'pf2/reporter'

class FirefoxProfilerSer2Test < Minitest::Test
  def test_empty
    report = Pf2::Reporter::FirefoxProfilerSer2.new({
      start_timestamp_ns: 1737730800000000,
      duration_ns: 15000000000,
      samples: [],
      locations: [],
      functions: [],
    }).emit

    assert_equal([], report[:threads])
  end

  def test_simple
    report = Pf2::Reporter::FirefoxProfilerSer2.new({
      start_timestamp_ns: 1737730800000000,
      duration_ns: 15000000000,
      samples: [
        { stack: [2, 1, 0], native_stack: [], ruby_thread_id: 1, elapsed_ns: 1000000, },
        { stack: [4, 3, 2, 1, 0], native_stack: [], ruby_thread_id: 1, elapsed_ns: 1000000, },
      ],
      locations: [
        { function_index: 0, lineno: 10, address: nil },
        { function_index: 1, lineno: 20, address: nil },
        { function_index: 2, lineno: 30, address: nil },
        { function_index: 3, lineno: 40, address: nil },
        { function_index: 4, lineno: 50, address: nil },
      ],
      functions: [
        { implementation: :ruby, name: 'foo', filename: 'main.rb', start_lineno: 10, start_address: nil },
        { implementation: :ruby, name: 'bar', filename: 'main.rb', start_lineno: 20, start_address: nil },
        { implementation: :ruby, name: 'baz', filename: 'main.rb', start_lineno: 30, start_address: nil },
        { implementation: :ruby, name: 'qux', filename: 'main.rb', start_lineno: 40, start_address: nil },
        { implementation: :ruby, name: 'quux', filename: 'main.rb', start_lineno: 50, start_address: nil },
      ],
    }).emit

    assert_equal(1, report[:threads].length)
    assert_equal(5, report[:threads][0][:stackTable][:length])
    assert_equal([0, 1, 2, 3, 4], report[:threads][0][:stackTable][:frame])
    assert_equal([nil, 0, 1, 2, 3], report[:threads][0][:stackTable][:prefix])
    assert_equal(5, report[:threads][0][:frameTable][:length])
    assert_equal(5, report[:threads][0][:funcTable][:length])
    assert_equal(['foo', 'main.rb', 'bar', 'baz', 'qux', 'quux'], report[:threads][0][:stringArray])
  end
end
