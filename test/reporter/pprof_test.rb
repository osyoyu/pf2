# frozen_string_literal: true

require 'minitest/autorun'
require 'stringio'
require 'zlib'
require_relative '../profile.proto'

require 'pf2'
require 'pf2/reporter'

class PprofReporterTest < Minitest::Test
  def test_empty_profile
    report = Pf2::Reporter::Pprof.new({
      start_timestamp_ns: 1737730800000000,
      duration_ns: 0,
      samples: [],
      locations: [],
      functions: [],
    }).emit

    profile = decode_profile(report)
    assert_equal 0, profile.sample.length
    assert_equal 0, profile.location.length
    assert_equal 0, profile.function.length
    strings = profile.string_table
    assert_equal '', strings.first
    assert_equal 'samples', strings[profile.sample_type[0].type]
  end

  def test_simple_profile
    profile = {
      start_timestamp_ns: 1737730800000000,
      duration_ns: 15000000000,
      samples: [
        { stack: [2, 1, 0], native_stack: [], ruby_thread_id: 1, elapsed_ns: 1_000_000 },
        { stack: [4, 3, 2, 1, 0], native_stack: [], ruby_thread_id: 1, elapsed_ns: 2_000_000 },
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
    }

    report = Pf2::Reporter::Pprof.new(profile).emit
    profile_pb = decode_profile(report)

    assert_equal 2, profile_pb.sample.length
    assert_equal 5, profile_pb.location.length
    assert_equal 5, profile_pb.function.length

    names = profile_pb.string_table
    %w[foo bar baz qux quux main.rb samples cpu nanoseconds].each do |expected|
      assert_includes(names, expected)
    end

    # Sample shape: leaf-first location ids, two values (count, cpu ns)
    first = profile_pb.sample.first
    assert_equal [1, 2, 3], first.location_id # corresponds to functions baz, bar, foo
    assert_equal [1, profile[:duration_ns] / profile[:samples].length], first.value

    second = profile_pb.sample.last
    assert_equal [1, 2, 3, 4, 5], second.location_id
    assert_equal [1, profile[:duration_ns] / profile[:samples].length], second.value

    # Location -> function mapping
    profile_pb.location.each_with_index do |loc, i|
      fn_id = loc.line.first.function_id
      fn = profile_pb.function.detect { |f| f.id == fn_id }
      expect_name = profile[:functions][i][:name]
      expect_file = profile[:functions][i][:filename]
      assert_equal expect_name, names[fn.name]
      assert_equal expect_file, names[fn.filename]
    end

    # Period fields
    assert_equal names[profile_pb.period_type.type], 'cpu'
    assert_equal names[profile_pb.period_type.unit], 'nanoseconds'
  end

  private

  def gunzip(bytes)
    bytes = bytes.dup.force_encoding(Encoding::BINARY)
    Zlib::GzipReader.new(StringIO.new(bytes)).read
  end

  def decode_profile(gz_bytes)
    Perftools::Profiles::Profile.decode(gunzip(gz_bytes))
  end
end
