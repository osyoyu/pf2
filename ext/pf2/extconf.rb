require 'mkmf'
require 'rb_sys/mkmf'

abort 'missing rb_profile_thread_frames()' unless have_func 'rb_profile_thread_frames'

create_rust_makefile 'pf2/pf2' do |r|
  if ENV['PF2_FEATURES']
    r.features = ENV['PF2_FEATURES'].split(",")
  end
end
