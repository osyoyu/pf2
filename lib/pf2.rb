require_relative 'pf2/pf2'
require_relative 'pf2/version'

module Pf2
  class Error < StandardError; end

  @@threads = []

  def self.threads
    @@threads
  end

  def self.threads=(th)
    @@threads = th
  end
end
