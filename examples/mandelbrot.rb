# mandelbrot
#
# Generate a Mandelbrot set image using multiple threads.

require 'bundler/inline'

gemfile do
  source 'https://rubygems.org'
  gem 'chunky_png'
end

require 'pf2'

def mandelbrot_pixel(x, y, width, height, max_iter)
  real_part = (x - width / 2.0) * 4.0 / width
  imag_part = (y - height / 2.0) * 4.0 / height

  c = Complex(real_part, imag_part)
  z = 0
  iter = 0

  while iter < max_iter && z.magnitude <= 2
    z = z * z + c
    iter += 1
  end

  iter
end

def generate_mandelbrot_image(width, height, max_iter, num_threads)
  image = ChunkyPNG::Image.new(width, height, ChunkyPNG::Color::TRANSPARENT)
  threads = []
  num_threads.times do |thread_id|
    threads << Thread.new(thread_id) do |tid|
      start_row = tid * (height / num_threads)
      end_row = (tid + 1) * (height / num_threads)

      (start_row...end_row).each do |y|
        width.times do |x|
          color_value = mandelbrot_pixel(x, y, width, height, max_iter)
          color = ChunkyPNG::Color.grayscale(color_value * 255 / max_iter)
          image[x, y] = color
        end
      end
    end
  end
  threads.each(&:join)
  image
end

# Parameters
width = 800
height = 800
max_iter = 1000
threads = 16

puts "width: #{width}, height: #{height}, max_iter: #{max_iter}, threads: #{threads}"

Pf2.start

start_time = Process.clock_gettime(Process::CLOCK_MONOTONIC)
generate_mandelbrot_image(width, height, max_iter, threads)
end_time = Process.clock_gettime(Process::CLOCK_MONOTONIC)

profile = Pf2.stop
File.binwrite("mandelbrot.pf2prof", Marshal.dump(profile))

elapsed = end_time - start_time
puts "Complete in #{elapsed} seconds"
