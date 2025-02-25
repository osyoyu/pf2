require 'bundler/gem_tasks'
require 'rake/extensiontask'
require 'minitest/test_task'

task default: %i[]

if ENV['PF2_PF2C'] == '1'
  Rake::ExtensionTask.new 'pf2c' do |ext|
    ext.name = 'pf2'
    ext.lib_dir = 'lib/pf2'
  end
else
  Rake::ExtensionTask.new 'pf2' do |ext|
    ext.lib_dir = 'lib/pf2'
  end
end

Minitest::TestTask.create(:test) do |t|
  t.libs << "test"
  t.libs << "lib"
  t.warning = false
  t.test_globs = ["test/**/*_test.rb"]
end
