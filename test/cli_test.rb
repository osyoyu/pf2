# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2/cli'

class CLITest < Minitest::Test
  def test_pf2_report_creates_output_file
    Tempfile.create("input.pf2") do |input_file|
      # Create an empty profile file
      empty_profile = { start_timestamp_ns: 0, duration_ns: 0, samples: [], locations: [], functions: [] }
      input_file.write(Marshal.dump(empty_profile))
      input_file.flush

      output_path = nil
      Dir::Tmpname.create("pf2_output.json") { |tmpname, _, _| output_path = tmpname }

      exit_code = Pf2::CLI.run(%W(report -o #{output_path} #{input_file.path}))

      assert_equal(0, exit_code)
      assert_equal(true, File.exist?(output_path))

      FileUtils.rm(output_path)
    end
  end
end
