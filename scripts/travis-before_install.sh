#!/bin/bash
set -ex
if [[ $TRAVIS_RUST_VERSION == "nightly" && $TRAVIS_BRANCH == "staging" ]]; then
  # Do not run bors builds against the nightly compiler.
  # We want to find out about nightly bugs, so they're done in master, but we don't block on them.
  exit
fi
if [[ "$TRAVIS_OS_NAME" == "linux" ]]; then
  export DISPLAY=:99.0
  sh -e /etc/init.d/xvfb start
  # Extract SDL2 .deb into a cached directory (see cache section above and LIBRARY_PATH exports below)
  # Will no longer be needed when Trusty build environment goes out of beta at Travis CI
  # (assuming it will have libsdl2-dev and Rust by then)
  # see https://docs.travis-ci.com/user/trusty-ci-environment/
  bash scripts/travis-install-sdl2.sh
elif [[ "$TRAVIS_OS_NAME" == "osx" ]]; then
  brew update
  brew install sdl2
  brew outdated cmake || brew upgrade cmake
fi
