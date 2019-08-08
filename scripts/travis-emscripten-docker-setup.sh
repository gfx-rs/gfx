#!/bin/bash

set -ev

# Start a container with the travis $HOME mounted at /root to get access to the rust installation,
# and the $TRAVIS_BUILD_DIR mounted to /src, (which is the container's initial working directory)
# so we can later run `cargo test ...` without changing directory.
DOCKER_BASE_PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
docker run -dit --name emscripten -e PATH=$DOCKER_BASE_PATH:/root/.cargo/bin -v $TRAVIS_BUILD_DIR:/src -v $HOME:/root trzeci/emscripten
