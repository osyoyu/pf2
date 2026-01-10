# frozen_string_literal: true

require 'pf2'

output_path = ENV.fetch('PF2_PROFILE_PATH')
duration_s = ENV.fetch('PF2_DURATION', '30').to_f
interval_ms = ENV.fetch('PF2_INTERVAL_MS', '1').to_i

deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + duration_s

profile = Pf2.profile(interval_ms: interval_ms, time_mode: :cpu) do
  x = 0
  while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline do
    x = (x + 1) & 0xffffffff
  end
  x
end

File.binwrite(output_path, Marshal.dump(profile))
