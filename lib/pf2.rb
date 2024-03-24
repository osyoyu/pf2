require_relative 'pf2/pf2'
require_relative 'pf2/session'
require_relative 'pf2/version'

module Pf2
  class Error < StandardError; end

  def self.start(...)
    @@session = Pf2::Session.new(...)
    @@session.start
  end

  def self.stop
    @@session.stop
  end

  def self.profile(&block)
    raise ArgumentError, "block required" unless block_given?
    start(threads: Thread.list)
    yield
    stop
    @@session = nil # let GC clean up the session
  end
end
