# frozen_string_literal: true

require 'minitest/autorun'

require_relative './test_helper.rb'

class ScriptCompletionTest < Minitest::Test
  def test_mandelbrot_in_main_thread_completes
    assert_equal(true, system_with_timeout('bundle exec ruby test/test_scripts/mandelbrot_in_main_thread.rb', 10).success?)
  end

  def test_mandelbrot_in_new_thread_completes
    assert_equal(true, system_with_timeout('bundle exec ruby test/test_scripts/mandelbrot_in_new_thread.rb', 10).success?)
  end

  def test_mandelbrot_in_multiple_threads_completes
    assert_equal(true, system_with_timeout('bundle exec ruby test/test_scripts/mandelbrot_in_multiple_threads.rb', 15).success?)
  end
end
