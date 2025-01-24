# frozen_string_literal: true

require 'minitest/autorun'

require 'pf2'
require 'pf2/reporter'

class FirefoxProfilerSer2Test < Minitest::Test
  def test_empty
    report = Pf2::Reporter::FirefoxProfilerSer2.new({
      start_timestamp_ns: 1737730800000000,
      duration_ns: 15000000000,
      samples: [],
      locations: [],
      functions: [],
    }).emit

    assert_equal([], report[:threads])
  end
end
