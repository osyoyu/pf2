# frozen_string_literal: true

require_relative 'pf2/pf2'
require_relative 'pf2/version'

module Pf2
  class Error < StandardError; end

  # Start a profiling session.
  #
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
  # Example:
  #
  #     profile = Pf2.profile(interval_ms: 42) do
  #       your_code_here
  #     end
  #
  def self.profile(**kwargs, &block)
    raise ArgumentError, "block required" unless block_given?
    start(**kwargs)
    yield
    result = stop
    @@session = nil # let GC clean up the session
    result
  ensure
    if defined?(@@session) && @@session != nil
      stop
      @@session = nil
    end
  end
end
