# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'
require 'pf2/reporter'

class FirefoxProfilerTest < Minitest::Test
  def test_empty
    report = Pf2::Reporter::FirefoxProfiler.new(
      {threads: {}}
    ).emit

    assert_equal([], report[:threads])
  end
end
