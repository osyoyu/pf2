# frozen_string_literal: true

require_relative 'pf2/pf2'
require_relative 'pf2/version'

module Pf2
  class Error < StandardError; end

  KNOWN_FORMATS = [:pf2prof, :firefox] # :nodoc:
  private_constant :KNOWN_FORMATS

  # Start a profiling session.
  #
  # Parameters:
  # interval_ms - Sampling interval in milliseconds (default: 9)
  # time_mode   - :cpu or :wall (default: :cpu)
  def self.start(...)
    @@session = Session.new(...)
    @@session.start
  end

  # Stop the current profiling session and return the profile data.
  def self.stop
    @@session.stop
  end

  # Profiles the given block of code.
  #
  # Parameters:
  # interval_ms - Sampling interval in milliseconds (default: 9)
  # time_mode   - :cpu or :wall (default: :cpu)
  # out         - String or IO-like object specifying where to write profile data.
  #                 - nil (default): do not write to file
  #                 - String: file path to write the profile data
  #                 - IO-like object: an object responding to #write
  # format      - Output format. Possible values are:
  #                 - :firefox (default): JSON for profiler.firefox.com
  #                 - :pf2prof: Raw profile dump loadable by 'pf2 report'
  #
  # Example:
  #
  #     profile = Pf2.profile(interval_ms: 42) do
  #       your_code_here
  #     end
  #
  def self.profile(interval_ms: 9, time_mode: :cpu, out: nil, format: :firefox, &block)
    raise ArgumentError, "block required" unless block_given?
    raise ArgumentError, "Unknown format: #{format}" unless KNOWN_FORMATS.include?(format)
    if !(out.nil? || out.is_a?(String) || (out.respond_to?(:write) && out.respond_to?(:close)))
      raise ArgumentError, "'out' must be an IO-like object"
    end

    start(interval_ms:, time_mode:)
    yield
    result = stop()
    @@session = nil # let GC clean up the session

    if out
      is_path_passed = out.is_a?(String)
      io = is_path_passed ? File.open(out, "wb") : out
      case format
      in :firefox
        require 'pf2/reporter'
        reporter = Reporter::FirefoxProfilerSer2.new(result)
        io.write(reporter.emit)
      in :pf2prof
        io.write(Marshal.dump(result))
      end
      io.close if is_path_passed
    end

    result
  ensure
    if defined?(@@session) && @@session != nil
      stop
      @@session = nil
    end
  end
end
