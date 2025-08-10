# frozen_string_literal: true

module Pf2
  module Reporter
    # "Weaves" the native stack into the Ruby stack.
    #
    # Strategy:
    # - Split the stack into Ruby and Native parts
    # - Start from the root of the Native stack
    # - Dig in to the native stack until we hit a rb_vm_exec(), which marks a call into Ruby code
    # - Switch to Ruby stack. Keep digging until we hit a Cfunc call, then switch back to Native stack
    # - Repeat until we consume the entire stack
    class StackWeaver
      def initialize(profile)
        @profile = profile
      end

      def weave(ruby_stack, native_stack)
        ruby_stack = ruby_stack.dup
        native_stack = native_stack.dup

        weaved_stack = []

        current_stack = :native
        loop do
          break if ruby_stack.size == 0 && native_stack.size == 0
          case current_stack
          when :ruby
            if ruby_stack.size == 0 # We've reached the end of the Ruby stack
              current_stack = :native
              next
            end

            location_index = ruby_stack.pop
            weaved_stack.unshift(location_index)

            current_stack = :native if should_switch_to_native?(location_index, native_stack.dup)

          when :native
            if native_stack.size == 0 # We've reached the end of the native stack
              current_stack = :ruby
              next
            end

            location_index = native_stack.pop
            weaved_stack.unshift(location_index)

            current_stack = :ruby if should_switch_to_ruby?(location_index)
          end
        end

        weaved_stack
      end

      # @param [Integer] location_index
      # @param [Array<Integer>] native_stack_remainder
      def should_switch_to_native?(location_index, native_stack_remainder)
        location = @profile[:locations][location_index]
        function = @profile[:functions][location[:function_index]]
        raise if function[:implementation] != :ruby # assert

        # Is the current Ruby function a cfunc?
        return false if function[:start_address] == nil

        # Does a corresponding native function exist in the remainder of the native stack?
        loop do
          break if native_stack_remainder.size == 0
          n_location_index = native_stack_remainder.pop
          n_location = @profile[:locations][n_location_index]
          n_function = @profile[:functions][n_location[:function_index]]

          return true if function[:start_address] == n_function[:start_address]
        end

        false
      end

      def should_switch_to_ruby?(location_index)
        location = @profile[:locations][location_index]
        function = @profile[:functions][location[:function_index]]
        raise if function[:implementation] != :native # assert

        # If the next function is a vm_exec_core() (= VM_EXEC in vm_exec.h),
        # we switch to the Ruby stack.
        function[:name] == 'vm_exec_core'
      end
    end
  end
end
