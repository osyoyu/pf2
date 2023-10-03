require 'mkmf'

abort 'missing rb_profile_thread_frames()' unless have_func 'rb_profile_thread_frames'

create_makefile 'pf2/pf2'
