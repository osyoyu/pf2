require_relative 'pf2/pf2'
require_relative 'pf2/version'

module Pf2
  class Error < StandardError; end

  @@collector = nil
  @@threads = []

  def self.start(...)
    @collector = Pf2::TimerCollector.new
    @collector.start(...)
  end

  def self.stop
    @collector.stop
  end

  def self.install_to_current_thread(...)
    @collector.install_to_current_thread(...)
  end

  def self.threads
    @@threads
  end

  def self.threads=(th)
    @@threads = th
  end
end
