# frozen_string_literal: true

module Pf2
  module Reporter
    class Annotate
      HitCount = Struct.new(:self, :total, keyword_init: false)
      SourceCodeHits = Struct.new(:path, :line_count, keyword_init: false)

      # @param profile [Hash]
      # @param source_directory [String]
      def initialize(profile, source_directory)
        @profile = profile
        @source_directory = source_directory
      end

      def annotate
        tallied = tally_by_source_code_line(@profile)

        # Print the source code with hit counts
        tallied.each do |path, source_code_hits|
          expanded_path = File.expand_path(path, @source_directory)
          if !File.exist?(expanded_path)
            if ignorable_path?(path)
              puts "Ignoring file: #{path}"
            else
              puts "File not found: #{path}"
            end
            puts ""
            puts ""
            next
          end
          source_file = File.open(expanded_path, "r")

          puts expanded_path
          puts ""

          # Print in tabular format

          # Header row
          puts "  ttl  self │"

          source_file.each_line.with_index(1) do |line, lineno|
            hits = source_code_hits.line_count[lineno]

            if !hits.nil?
              # If any samples are captured for this line
              puts "%5d %5d │ %s" % [hits.total, hits.self, line.chomp]
            else
              puts "%5s %5s │ %s" % ["", "", line.chomp]
            end
          end

          puts ""
          puts ""
        ensure
          source_file.close if source_file
        end
      end

      # @return [Array<SourceCodeHits>]
      private def tally_by_source_code_line(profile)
        # Iterate over all samples and tally self hits and total hits by location
        hits_per_location = {}
        @profile[:samples].each do |sample|
          weight = sample[:count] || 1
          # Record a total hit for all locations in the stack
          sample[:stack].each do |location_id|
            hits_per_location[location_id] ||= HitCount.new(0, 0)
            hits_per_location[location_id].total += weight
          end

          # Record a self hit for the topmost stack frame, which is the first element in the array
          topmost_location_id = sample[:stack][0]
          hits_per_location[topmost_location_id].self += weight
        end

        # Associate a filename and lineno for each location
        hits_per_file = {}
        hits_per_location.each do |location_id, hits|
          location = @profile[:locations][location_id]
          function = @profile[:functions][location[:function_index]]

          filename = function[:filename]
          # Some locations simply cannot be associated to a specific file.
          # We just ignore them.
          next if filename.nil?
          lineno = location[:lineno]

          hits_per_file[filename] ||= SourceCodeHits.new(filename, {})
          hits_per_file[filename].line_count[lineno] = hits
        end

        hits_per_file
      end

      private def ignorable_path?(path)
        return true if path.start_with?("<internal:")
        false
      end
    end
  end
end
