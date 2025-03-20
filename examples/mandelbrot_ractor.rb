# mandelbrot_ractor
#
# This script demonstrates how to profile a Ruby program that uses Ractors.

require 'bundler/inline'

gemfile do
  source 'https://rubygems.org'
  gem 'chunky_png'
end

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

def generate_mandelbrot_image(width, height, max_iter, num_ractors)
  ractors = []
  num_ractors.times do |ractor_id|
    ractors << Ractor.new(width, height, max_iter, num_ractors, ractor_id) do |width, height, max_iter, num_ractors, rid|
      image = ChunkyPNG::Image.new(width, height, ChunkyPNG::Color::TRANSPARENT)

      start_row = rid * (height / num_ractors)
      end_row = (rid + 1) * (height / num_ractors)

      (start_row...end_row).each do |y|
        width.times do |x|
          color_value = mandelbrot_pixel(x, y, width, height, max_iter)
          color = ChunkyPNG::Color.grayscale(color_value * 255 / max_iter)
          image[x, y] = color
        end
      end

      Ractor.yield image
    end
  end
  image_parts = ractors.map(&:take)

  # Merge image_parts into a single image
  image = ChunkyPNG::Image.new(width, height, ChunkyPNG::Color::TRANSPARENT)
  image_parts.each do |image_part|
    image_part.height.times do |y|
      image_part.width.times do |x|
        if !image_part[x, y].nil?
          image[x, y] = image_part[x, y]
        end
      end
    end
  end
  image
end

# Parameters
width = 800
height = 800
max_iter = 1000
ractors = 4

puts "width: #{width}, height: #{height}, max_iter: #{max_iter}, ractors: #{ractors}"

start_time = Process.clock_gettime(Process::CLOCK_MONOTONIC)
generate_mandelbrot_image(width, height, max_iter, ractors)
end_time = Process.clock_gettime(Process::CLOCK_MONOTONIC)

elapsed = end_time - start_time
puts "Complete in #{elapsed} seconds"
