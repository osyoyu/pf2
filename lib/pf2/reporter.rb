require 'json'

module Pf2
  # Generates Firefox Profiler's "processed profile format"
  # https://github.com/firefox-devtools/profiler/blob/main/docs-developer/processed-profile-format.md
  class Reporter
    def initialize(profile)
      @profile = profile
    end

    def inspect
      "" # TODO: provide something better
    end

    def emit
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
              color: "lightblue",
              subcategories: ["Code"],
            },
          ],
          marker_schema: [],
        },
        libs: [],
        counters: [],
        threads: @profile[:threads].values.map {|th| ThreadReport.new(th).emit }
      }
      Reporter.deep_camelize_keys(report)
    end

    class ThreadReport
      def initialize(thread)
        @thread = thread

        # Populated in other methods
        @func_id_map = {}
        @frame_id_map = {}
        @stack_tree_id_map = {}

        @string_table = {}
      end

      def inspect
        "" # TODO: provide something better
      end

      def emit
        func_table = build_func_table
        frame_table = build_frame_table
        stack_table = build_stack_table
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
          name: "Thread (tid: #{@thread[:thread_id]})",
          is_main_thread: true,
          is_js_tracer: true,
          # FIXME: We can fill the correct PID only after we correctly fill is_main_thread
          # (only one thread could be marked as is_main_thread in a single process)
          pid: @thread[:thread_id],
          tid: @thread[:thread_id],
          samples: samples,
          markers: markers,
          stack_table: stack_table,
          frame_table: frame_table,
          string_array: build_string_table,
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

        @thread[:samples].each do |sample|
          ret[:stack] << @stack_tree_id_map[sample[:stack_tree_id]]
          ret[:time] << sample[:elapsed_ns] / 1000000 # ns -> ms
          ret[:duration] << 1
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

        @thread[:frames].each.with_index do |(id, frame), i|
          ret[:address] << nil
          ret[:category] << 1
          ret[:subcategory] << 1
          ret[:func] << i # TODO
          ret[:inner_window_id] << nil
          ret[:implementation] << nil
          ret[:line] << frame[:lineno]
          ret[:column] << nil
          ret[:optimizations] << nil
          ret[:inline_depth] << 0
          ret[:native_symbol] << nil

          @frame_id_map[id.to_s] = i
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

        @thread[:frames].each.with_index do |(id, frame), i|
          ret[:name] << string_id("#{frame[:full_label]} (#{frame[:path]}:#{frame[:lineno]})")
          ret[:is_js] << false
          ret[:relevant_for_js] << false
          ret[:resource] << -1
          ret[:file_name] << nil
          ret[:line_number] << nil
          ret[:column_number] << nil

          @func_id_map[id] = i
        end

        ret[:length] = ret[:name].length
        ret
      end

      def build_stack_table
        ret = {
          frame: [],
          category: [],
          subcategory: [],
          prefix: [],
        }

        queue = []

        @thread[:stack_tree][:children].each {|_, c| queue << [nil, c] }

        loop do
          break if queue.size == 0

          prefix, node = queue.shift
          ret[:frame] << @frame_id_map[node[:frame_id]]
          ret[:category] << 1
          ret[:subcategory] << nil
          ret[:prefix] << prefix

          # The index of this frame - children can refer to this frame using this index as prefix
          frame_index = ret[:frame].length - 1
          @stack_tree_id_map[node[:node_id]] = frame_index

          # Enqueue children nodes
          node[:children].each {|_, c| queue << [frame_index, c] }
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

      def deep_intize_keys(value)
        deep_transform_keys(value) do |key|
          if key.to_s.to_i.to_s == key.to_s
            key.to_s.to_i
          else
            key
          end
        end
      end
    end
  end
end
