# frozen_string_literal: true

require 'minitest/autorun'
require 'json'
require 'stringio'
require 'tempfile'
require 'tmpdir'
require 'zlib'

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

  def test_profile_writes_firefox_report_to_io
    Tempfile.create do |file|
      file.close # Pf2.profile will open by path
      Pf2.profile(out: file.path, format: :firefox) { 1 + 1 }
      parsed = JSON.parse(File.read(file.path))

      assert_kind_of Hash, parsed
      assert parsed.key?('threads')
    end
  end

  def test_profile_writes_firefox_report_to_stringio
    io = StringIO.new(+'', 'r+')
    Pf2.profile(out: io, format: :firefox) { 1 + 1 }
    io.rewind
    parsed = JSON.parse(io.read)

    assert_kind_of Hash, parsed
    assert parsed.key?('threads')
  end

  def test_profile_writes_pf2prof_report_to_path
    Dir.mktmpdir do |dir|
      path = File.join(dir, 'profile.pf2prof')
      profile = Pf2.profile(out: path, format: :pf2prof) { 1 + 1 }
      written = File.binread(path)
      assert_equal profile, Marshal.load(written)
    end
  end

  def test_profile_writes_pprof_report_to_path
    Dir.mktmpdir do |dir|
      path = File.join(dir, 'profile.pprof')
      Pf2.profile(out: path, format: :pprof) { 1 + 1 }
      written = File.binread(path)
      # gzip magic bytes 1F 8B
      assert_equal "\x1F\x8B".b, written.byteslice(0, 2)
    end
  end

  def test_profile_rejects_non_io_out
    assert_raises(ArgumentError, "'out' must be an IO-like object") do
      Pf2.profile(out: 42) { 1 + 1 }
    end
  end

  def test_profile_raises_error_for_unknown_format
    assert_raises(ArgumentError, "Unknown format: :invalid") do
      Pf2.profile(format: :invalid) { 1 + 1 }
    end
  end
end
