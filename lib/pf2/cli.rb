require 'optparse'

require 'pf2'
require 'pf2/reporter'

module Pf2
  class CLI
    def self.run(...)
      new.run(...)
    end

    def run(argv)
      argv = argv.dup
      program_name = File.basename($PROGRAM_NAME)

      subcommand = argv.shift
      case subcommand
      when 'report'
        subcommand_report(argv)
      when 'serve'
        subcommand_serve(argv)
      when 'version'
        puts VERSION
        return 0
      when '--help'
        STDERR.puts <<~__EOS__
        Usage: #{program_name} COMMAND [options]

        Commands:
          report    Generate a report from a profile
          serve     Start an HTTP server alongside a target process
          version   Show version information
        __EOS__

        return 1
      else
        STDERR.puts "#{program_name}: Unknown subcommand '#{subcommand}'."
        STDERR.puts "See '#{program_name} --help'"
        return 1
      end
    end

    def subcommand_report(argv)
      options = {}
      option_parser = OptionParser.new do |opts|
        opts.banner = "Usage: pf2 report [options] COMMAND"
        opts.on('-h', '--help', 'Prints this help') do
          puts opts
          return 0
        end
        opts.on('-o', '--output FILE', 'Output file') do |path|
          options[:output_file] = path
        end
      end
      option_parser.parse!(argv)

      profile = JSON.parse(File.read(argv[0]), symbolize_names: true, max_nesting: false)
      report = JSON.generate(Pf2::Reporter::FirefoxProfiler.new(profile).emit)

      if options[:output_file]
        File.write(options[:output_file], report)
      else
        puts report
      end

      return 0
    end

    def subcommand_serve(argv)
      options = {}
      option_parser = OptionParser.new do |opts|
        opts.banner = "Usage: pf2 serve [options] COMMAND"
        opts.on('-h', '--help', 'Prints this help') do
          puts opts
          return 0
        end
        opts.on('-b', '--bind ADDRESS', 'Address to bind') do |host|
          options[:serve_host] = host
        end
        opts.on('-p', '--port PORT', '') do |port|
          options[:serve_port] = port
        end
      end
      option_parser.parse!(argv)

      if argv.size == 0
        # No subcommand was specified
        STDERR.puts option_parser.help
        return 1
      end

      # Inject the profiler (pf2/serve) into the target process via RUBYOPT (-r).
      # This will have no effect if the target process is not Ruby.
      env = {
        'RUBYOPT' => '-rpf2/serve'
      }
      env['PF2_SERVE_HOST'] = options[:serve_host] if options[:serve_host]
      env['PF2_SERVE_PORT'] = options[:serve_port] if options[:serve_port]
      exec(env, *argv) # never returns if succesful

      return 1
    end
  end
end
