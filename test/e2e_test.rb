# frozen_string_literal: true

require 'minitest/autorun'
require 'tmpdir'

require_relative './test_helper.rb'

class E2ETest < Minitest::Test
  def test_tarai_in_main
    run_script_and_verify_profile('tarai_in_main_thread.rb', expected_methods: ['Object#tarai'])
  end

  def test_tarai_in_new_thread
    run_script_and_verify_profile('tarai_in_new_thread.rb', expected_methods: ['Object#tarai'])
  end

  def test_tarai_in_multiple_threads
    run_script_and_verify_profile('tarai_in_multiple_threads.rb', expected_methods: ['Object#tarai'])
  end

  # Assert Ruby does not crash even if the script exits while profiling is active.
  def test_exit_while_profiling_completes
    assert_equal true, system_with_timeout('bundle exec ruby test/test_scripts/exit_while_profiling.rb', 3).success?
  end

  private

  def run_script_and_verify_profile(script, expected_methods:)
    Dir.mktmpdir("pf2_test_") do |tmpdir|
      script_path = File.expand_path(File.join(__dir__, "test_scripts", script))
      profile_path = File.join(tmpdir, "#{File.basename(script, ".rb")}.pf2prof")
      result = system_with_timeout(
        "bundle exec ruby #{script_path}",
        5,
        env: { "PF2_PROFILE_PATH" => profile_path }
      )
      assert_equal(true, result.success?, "Script failed: #{script}")

      verify_profile(profile_path, expected_methods)
    end
  end

  def verify_profile(profile_path, expected_methods)
    profile = Marshal.load(File.binread(profile_path))
    assert_instance_of Hash, profile

    samples = profile[:samples]
    refute_empty samples

    functions = profile[:functions]
    refute_empty functions

    function_names = functions.map {|f| f[:name] }.uniq.compact
    expected_methods.each do |method_name|
      assert function_names.include?(method_name), "Expected method not found in profile: #{method_name}"
    end
  end
end
