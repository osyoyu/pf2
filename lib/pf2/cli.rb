require 'optparse'

require 'pf2'
require 'pf2/reporter'

module Pf2
  class CLI
    def self.run(...)
      new.run(...)
    end

    def run(argv)
      options = {}
      option_parser = OptionParser.new do |opts|
        opts.on('-v', '--version', 'Prints version') do
          puts Pf2::VERSION
          exit
        end

        opts.on('-h', '--help', 'Prints this help') do
          puts opts
        end

        opts.on('-o', '--output FILE', 'Output file') do |path|
          options[:output_file] = path
        end
      end
      option_parser.parse!(argv)

      profile = JSON.parse(File.read(ARGV[0]), symbolize_names: true)
      report = JSON.generate(Pf2::Reporter.new(profile).emit)

      if options[:output_file]
        File.write(options[:output_file], report)
      else
        puts report
      end

      return 0
    end
  end
end
