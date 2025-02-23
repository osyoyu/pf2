require 'mkmf'

append_cflags('-fvisibility=hidden')
append_cflags('-DPF2_DEBUG') # TODO: make this conditional

if have_func('timer_create')
  $srcs = Dir.glob("#{File.join(File.dirname(__FILE__), '*.c')}")
  create_makefile 'pf2/pf2c'
end
