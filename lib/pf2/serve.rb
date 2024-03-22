require 'json'
require 'logger'
require 'uri'
require 'webrick'

require_relative '../pf2'
require_relative './reporter'

module Pf2
  class Serve
    CONFIG = {
      Host: ENV.fetch('PF2_SERVE_HOST', 'localhost'),
      Port: ENV.fetch('PF2_SERVE_PORT', '51502').to_i, # 1502 = 0xF2 (as in "Pf2")
      Logger: Logger.new(nil),
      AccessLog: [],
    }

    def self.start

      # Ignore Bundler as in `bundle exec`.
      if File.basename($PROGRAM_NAME) == 'bundle' && ARGV.first == 'exec'
        return
      end

      server = WEBrick::HTTPServer.new(CONFIG)
      server.mount_proc('/profile') do |req, res|
        profile = Pf2.stop
        profile = JSON.parse(profile, symbolize_names: true, max_nesting: false)
        res.header['Content-Type'] = 'application/json'
        res.header['Access-Control-Allow-Origin'] = '*'
        res.body = JSON.generate(Pf2::Reporter.new((profile)).emit)
        Pf2.start
      end

      Pf2.start

      Thread.new do
        hostport = "#{server.config[:Host]}:#{server.config[:Port]}"
        # Print host:port to trigger VS Code's auto port-forwarding feature
        STDERR.puts "[Pf2] Listening on #{hostport}."
        STDERR.puts "[Pf2] Open https://profiler.firefox.com/from-url/#{URI.encode_www_form_component("http://#{hostport}/profile")} for visualization."
        STDERR.puts ""
        server.start
      end
    end

    def self.at_exit
      STDERR.puts ""
      STDERR.puts "[Pf2] Script execution complete (Pf2 server is still listening). Hit Ctrl-C to quit."

      # Allow the user to download the profile after the target program exits
      sleep
    end
  end
end

Pf2::Serve.start
at_exit do
  Pf2::Serve.at_exit
end
