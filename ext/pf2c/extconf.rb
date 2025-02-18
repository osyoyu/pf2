require 'mkmf'

append_cflags('-fvisibility=hidden')

create_makefile 'pf2/pf2c'
