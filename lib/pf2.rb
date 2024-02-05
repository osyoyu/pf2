require_relative 'pf2/pf2'
require_relative 'pf2/version'

module Pf2
  class Error < StandardError; end

  def self.default_scheduler_class
    # SignalScheduler is Linux-only. Use TimerThreadScheduler on other platforms.
    if defined?(SignalScheduler)
      SignalScheduler
    else
      TimerThreadScheduler
    end
  end

  def self.start(...)
    @@default_scheduler = default_scheduler_class.new(...)
    @@default_scheduler.start
  end

  def self.stop(...)
    @@default_scheduler.stop(...)
  end

  def self.profile(&block)
    raise ArgumentError, "block required" unless block_given?
    start([Thread.current], true)
    yield
    stop
  end
end
