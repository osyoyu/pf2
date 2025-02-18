require 'mkmf'

append_cflags('-fvisibility=hidden')
append_cflags('-DPF2_DEBUG') # TODO: make this conditional

create_makefile 'pf2/pf2c'
