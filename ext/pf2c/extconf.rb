require 'mkmf'
require 'mini_portile2'

libbacktrace = MiniPortile.new('libbacktrace', '1.0.0')
libbacktrace.source_directory = File.expand_path(File.join(File.dirname(__FILE__), '..', '..', 'vendor', 'libbacktrace'))
libbacktrace.configure_options << 'CFLAGS=-fPIC'
libbacktrace.cook
libbacktrace.mkmf_config

if !have_func('backtrace_full', 'backtrace.h')
  raise 'libbacktrace has not been properly configured'
end

append_ldflags('-lrt') # for timer_create
append_cflags('-fvisibility=hidden')
append_cflags('-DPF2_DEBUG') # TODO: make this conditional

if have_func('timer_create')
  $srcs = Dir.glob("#{File.join(File.dirname(__FILE__), '*.c')}")
  create_makefile 'pf2/pf2'
end
