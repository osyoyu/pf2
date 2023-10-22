require_relative 'lib/pf2/version'

Gem::Specification.new do |spec|
  spec.name = 'pf2'
  spec.version = Pf2::VERSION
  spec.authors = ['Daisuke Aritomo']
  spec.email = ['osyoyu@osyoyu.com']

  spec.summary = 'Yet another Ruby profiler'
  spec.homepage = 'https://github.com/osyoyu/pf2'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 3.3.0.dev'

  spec.extensions = ['ext/pf2/extconf.rb']

  spec.metadata['allowed_push_host'] = 'https://rubygems.org'

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/osyoyu/pf2'
  spec.metadata['changelog_uri'] = 'https://github.com/osyoyu/pf2/blob/master/CHANGELOG.md'

  # Specify which files should be added to the gem when it is released.
  # The `git ls-files -z` loads the files in the RubyGem that have been added into git.
  spec.files = Dir.chdir(__dir__) do
    `git ls-files -z`.split("\x0").reject do |f|
      (File.expand_path(f) == __FILE__) ||
        f.start_with?(*%w[bin/ test/ spec/ .git Gemfile])
    end
  end
  spec.bindir = 'exe'
  spec.executables = spec.files.grep(%r{\Aexe/}) { |f| File.basename(f) }
  spec.require_paths = ['lib']

  spec.add_dependency 'rake-compiler'
  spec.add_dependency 'rb_sys', '~> 0.9.63'

  # For more information and examples about making a new gem, check out our
  # guide at: https://bundler.io/guides/creating_gem.html
end
