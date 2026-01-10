# frozen_string_literal: true

require 'pf2'

output_path = ENV.fetch('PF2_PROFILE_PATH')
duration_s = ENV.fetch('PF2_DURATION', '5').to_f
interval_ms = ENV.fetch('PF2_INTERVAL_MS', '1').to_i

start_time = Process.clock_gettime(Process::CLOCK_MONOTONIC)

profile = Pf2.profile(interval_ms: interval_ms, time_mode: :cpu) do
  acc = 0
  while Process.clock_gettime(Process::CLOCK_MONOTONIC) - start_time < duration_s
    10_000.times do
      acc = (acc * 1_664_525 + 1_013_904_223) & 0xffffffff
    end
  end
  acc
end

File.binwrite(output_path, Marshal.dump(profile))
