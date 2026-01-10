# frozen_string_literal: true

require 'pf2'

def tarai(x, y, z)
  x <= y ? y : tarai(tarai(x - 1, y, z), tarai(y - 1, z, x), tarai(z - 1, x, y))
end

output_path = ENV.fetch("PF2_PROFILE_PATH")

profile = Pf2.profile {
  tarai(12, 9, 1)
}

File.binwrite(output_path, Marshal.dump(profile))
