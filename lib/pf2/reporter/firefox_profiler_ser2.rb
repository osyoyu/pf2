# frozen_string_literal: true

require 'json'

module Pf2
  module Reporter
    # Generates Firefox Profiler's "processed profile format"
    # https://github.com/firefox-devtools/profiler/blob/main/docs-developer/processed-profile-format.md
    class FirefoxProfilerSer2
      def initialize(profile)
        @profile = profile
      end

      def inspect
        "#<#{self.class.name}>" # TODO: add sample count etc.
      end

      def emit
        grouped_threads = @profile[:samples].group_by {|s| s[:ruby_thread_id] }
        thread_reports = grouped_threads.map do |thread_id, samples|
          ThreadReport.new(@profile, thread_id, samples).emit
        end

        report = {
          meta: {
            interval: 10, # ms; TODO: replace with actual interval
            start_time: 0,
            process_type: 0,
            product: 'ruby',
            stackwalk: 0,
            version: 28,
            preprocessed_profile_version: 47,
            symbolicated: true,
            categories: [
              {
                name: "Logs",
                color: "grey",
                subcategories: ["Unused"],
              },
              {
                name: "Ruby",
                color: "red",
                subcategories: ["Code"],
              },
              {
                name: "Native",
                color: "blue",
                subcategories: ["Code"],
              },
              {
                name: "Native",
                color: "lightblue",
                subcategories: ["Code"],
              },
            ],
            marker_schema: [],
          },
          libs: [],
          counters: [],
          threads: thread_reports,
        }
        FirefoxProfiler.deep_camelize_keys(report)
      end

      class ThreadReport
        def initialize(profile, thread_id, samples)
          @profile = profile
          @thread_id = thread_id
          @samples = samples

          # Global state
          @seq = 0
          @stack_tree = { :stack_id => nil }
          @reverse_stack_tree = []
          @string_table = {}
        end

        def inspect
          "" # TODO: provide something better
        end

        def emit
          # TODO: weave?
          # @thread[:stack_tree] = x

          # Build func table from profile[:functions]
          func_table = build_func_table
          # Build frame table from profile[:locations]
          frame_table = build_frame_table
          # Build stack table from profile[:samples][][:stack]
          stack_table = build_stack_table(func_table, frame_table)
          # Build samples from profile[:samples]
          samples = build_samples

          string_table = build_string_table

          {
            process_type: 'default',
            process_name: 'ruby',
            process_startup_time: 0,
            process_shutdown_time: nil,
            register_time: 0,
            unregister_time: nil,
            paused_ranges: [],
            name: "Thread (tid: #{@thread_id})",
            is_main_thread: true,
            is_js_tracer: true,
            # FIXME: We can fill the correct PID only after we correctly fill is_main_thread
            # (only one thread could be marked as is_main_thread in a single process)
            pid: @thread_id,
            tid: @thread_id,
            samples: samples,
            markers: markers,
            stack_table: stack_table,
            frame_table: frame_table,
            string_array: string_table,
            func_table: func_table,
            resource_table: {
              lib: [],
              name: [],
              host: [],
              type: [],
              length: 0,
            },
            native_symbols: [],
          }
        end

        def build_samples
          ret = {
            event_delay: [],
            stack: [],
            time: [],
            duration: [],
            # weight: nil,
            # weight_type: 'samples',
          }

          @samples.each do |sample|
            stack = [*sample[:stack].reverse]
            stack_id = @stack_tree.dig(*stack, :stack_id)

            ret[:stack] << stack_id
            ret[:time] << sample[:elapsed_ns] / 1_000_000 # ns -> ms
            ret[:duration] << 100
            ret[:event_delay] << 0
          end

          ret[:length] = ret[:stack].length
          ret
        end

        def build_frame_table
          ret = {
            address: [],
            category: [],
            subcategory: [],
            func: [],
            inner_window_id: [],
            implementation: [],
            line: [],
            column: [],
            optimizations: [],
            inline_depth: [],
            native_symbol: [],
          }

          @profile[:locations].each.with_index do |location, i|
            ret[:address] << location[:address]
            ret[:category] << 1
            ret[:subcategory] << 1
            ret[:func] << location[:function_index]
            ret[:inner_window_id] << nil
            ret[:implementation] << nil
            ret[:line] << location[:lineno]
            ret[:column] << nil
            ret[:optimizations] << nil
            ret[:inline_depth] << 0
            ret[:native_symbol] << nil
          end

          ret[:length] = ret[:address].length
          ret
        end

        def build_func_table
          ret = {
            name: [],
            is_js: [],
            relevant_for_js: [],
            resource: [],
            file_name: [],
            line_number: [],
            column_number: [],
          }

          @profile[:functions].each.with_index do |function, i|
            is_ruby = (function[:implementation] == :ruby)

            ret[:name] << string_id(function[:name])
            ret[:is_js] << is_ruby
            ret[:relevant_for_js] << false
            ret[:resource] << -1
            ret[:file_name] << string_id(function[:filename])
            ret[:line_number] << function[:start_lineno]
            ret[:column_number] << nil
          end

          ret[:length] = ret[:name].length
          ret
        end

        def build_stack_table(func_table, frame_table)
          ret = {
            frame: [],
            category: [],
            subcategory: [],
            prefix: [],
          }

          @profile[:samples].each do |sample|
            # Stack (Array of location indices) recorded in sample, reversed
            # example: [1, 2, 9] (1 is the root)
            stack = [*sample[:stack].reverse]

            # Traverse the stack tree
            ptr = @stack_tree # pointer to the stack tree root
            stack.each do |location_index|
              # Register the stack if it's not already registered
              if ptr[location_index].nil?
                # Assign a new stack id for the current stack
                new_stack_id = ret[:frame].length # The position of the new stack in the stack_table array
                ptr[location_index] = { stack_id: new_stack_id }

                location = @profile[:locations][location_index]
                function = @profile[:functions][location[:function_index]]

                ret[:frame] << location[:function_index]
                ret[:category] << (function[:implementation] == :ruby ? 2 : 1)
                ret[:subcategory] << nil
                ret[:prefix] << ptr[:stack_id]
              end

              ptr = ptr[location_index]
            end
          end

          ret[:length] = ret[:frame].length
          ret
        end

        def build_string_table
          @string_table.sort_by {|_, v| v}.map {|s| s[0] }
        end

        def string_id(str)
          return @string_table[str] if @string_table.has_key?(str)
          @string_table[str] = @string_table.length
          @string_table[str]
        end

        def incr
          @seq += 1
        end

        def markers
          {
            data: [],
            name: [],
            time: [],
            start_time: [],
            end_time: [],
            phase: [],
            category: [],
            length: 0
          }
        end
      end

      # Util functions
      class << self
        def snake_to_camel(s)
          return "isJS" if s == "is_js"
          return "relevantForJS" if s == "relevant_for_js"
          return "innerWindowID" if s == "inner_window_id"
          s.split('_').inject([]) {|buffer, p| buffer.push(buffer.size == 0 ? p : p.capitalize) }.join
        end

        def deep_transform_keys(value, &block)
          case value
          when Array
            value.map {|v| deep_transform_keys(v, &block) }
          when Hash
            Hash[value.map {|k, v| [yield(k), deep_transform_keys(v, &block)] }]
          else
            value
          end
        end

        def deep_camelize_keys(value)
          deep_transform_keys(value) do |key|
            snake_to_camel(key.to_s).to_sym
          end
        end
      end
    end
  end
end
