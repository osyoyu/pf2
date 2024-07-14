require 'json'

module Pf2
  module Reporter
    # Generates Firefox Profiler's "processed profile format"
    # https://github.com/firefox-devtools/profiler/blob/main/docs-developer/processed-profile-format.md
    class FirefoxProfiler
      def initialize(profile)
        @profile = FirefoxProfiler.deep_intize_keys(profile)
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
          threads: @profile[:threads].values.map {|th| ThreadReport.new(th).emit }
        }
        FirefoxProfiler.deep_camelize_keys(report)
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
          x = weave_native_stack(@thread[:stack_tree])
          @thread[:stack_tree] = x
          func_table = build_func_table
          frame_table = build_frame_table
          stack_table = build_stack_table(func_table, frame_table)
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
            ret[:address] << frame[:address].to_s
            ret[:category] << 1
            ret[:subcategory] << 1
            ret[:func] << i # TODO
            ret[:inner_window_id] << nil
            ret[:implementation] << nil
            ret[:line] << frame[:callsite_lineno]
            ret[:column] << nil
            ret[:optimizations] << nil
            ret[:inline_depth] << 0
            ret[:native_symbol] << nil

            @frame_id_map[id] = i
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
            native = (frame[:entry_type] == 'Native')
            label = "#{native ? "Native: " : ""}#{frame[:full_label]}"
            ret[:name] << string_id(label)
            ret[:is_js] << !native
            ret[:relevant_for_js] << false
            ret[:resource] << -1
            ret[:file_name] << string_id(frame[:file_name])
            ret[:line_number] << frame[:function_first_lineno]
            ret[:column_number] << nil

            @func_id_map[id] = i
          end

          ret[:length] = ret[:name].length
          ret
        end

        # "Weave" the native stack into the Ruby stack.
        #
        # Strategy:
        # - Split the stack into Ruby and Native parts
        # - Start from the root of the Native stack
        # - Dig in to the native stack until we hit a rb_vm_exec(), which marks a call into Ruby code
        # - Switch to Ruby stack. Keep digging until we hit a Cfunc call, then switch back to Native stack
        # - Repeat until we consume the entire stack
        def weave_native_stack(stack_tree)
          collected_paths = []
          tree_to_array_of_paths(stack_tree, @thread[:frames], [], collected_paths)
          collected_paths = collected_paths.map do |path|
            next if path.size == 0

            new_path = []
            new_path << path.shift # root

            # Split the stack into Ruby and Native parts
            native_path, ruby_path = path.partition do |frame|
              frame_id = frame[:frame_id]
              @thread[:frames][frame_id][:entry_type] == 'Native'
            end

            mode = :native

            loop do
              break if ruby_path.size == 0 && native_path.size == 0

              case mode
              when :ruby
                if ruby_path.size == 0
                  mode = :native
                  next
                end

                next_node = ruby_path[0]
                new_path << ruby_path.shift
                next_node_frame = @thread[:frames][next_node[:frame_id]]
                if native_path.size > 0
                  # Search the remainder of the native stack for the same address
                  # Note: This isn't a very efficient way for the job... but it still works
                  ruby_addr = next_node_frame[:address]
                  native_path[0..].each do |native_node|
                    native_addr = @thread[:frames][native_node[:frame_id]][:address]
                    if ruby_addr && native_addr && ruby_addr == native_addr
                      # A match has been found. Switch to native mode
                      mode = :native
                      break
                    end
                  end
                end
              when :native
                if native_path.size == 0
                  mode = :ruby
                  next
                end

                # Dig until we meet a rb_vm_exec
                next_node = native_path[0]
                new_path << native_path.shift
                if @thread[:frames][next_node[:frame_id]][:full_label] =~ /vm_exec_core/ # VM_EXEC in vm_exec.h
                  mode = :ruby
                end
              end
            end

            new_path
          end

          # reconstruct stack_tree
          new_stack_tree = array_of_paths_to_tree(collected_paths)
          new_stack_tree
        end

        def tree_to_array_of_paths(stack_tree, frames, path, collected_paths)
          new_path = path + [{ frame_id: stack_tree[:frame_id], node_id: stack_tree[:node_id] }]
          if stack_tree[:children].empty?
            collected_paths << new_path
          else
            stack_tree[:children].each do |frame_id, child|
              tree_to_array_of_paths(child, frames, new_path, collected_paths)
            end
          end
        end

        def array_of_paths_to_tree(paths)
          new_stack_tree = { children: {}, node_id: 0, frame_id: 0 }
          paths.each do |path|
            current = new_stack_tree
            path[1..].each do |frame|
              frame_id = frame[:frame_id]
              node_id = frame[:node_id]
              current[:children][frame_id] ||= { children: {}, node_id: node_id, frame_id: frame_id }
              current = current[:children][frame_id]
            end
          end
          new_stack_tree
        end

        def build_stack_table(func_table, frame_table)
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
            ret[:category] << (build_string_table[func_table[:name][frame_table[:func][@frame_id_map[node[:frame_id]]]]].start_with?('Native:') ? 2 : 1)
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
end
