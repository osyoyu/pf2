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

Pf2.start(threads: [Thread.current])
takeuchi(14, 10, 1)
Pf2.stop
