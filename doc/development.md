Pf2 Development
===========

Setup
--------

- `git submodule update --init`


Releasing
--------

- Update CHANGELOG.md
- Update version in lib/pf2/version.rb
- Run `bundle install` to update Gemfile.lock
- Commit changes
- Run `bundle exec rake release`
