# frozen_string_literal: true

require 'pf2'

def takeuchi(x, y, z)
  if x <= y
    y
  else
    takeuchi(
      takeuchi(x - 1, y, z),
      takeuchi(y - 1, z, x),
      takeuchi(z - 1, x, y)
    )
  end
end

ths = 2.times.map do
  Thread.new { takeuchi(14, 10, 1) }
end
Pf2.start
ths.each { |th| th.join }
Pf2.stop
