#!/bin/bash

set -ev

DEST=$HOME/deps

-d $DEST/usr/bin/sdl2-config && exit

mkdir -p $DEST
cd $DEST

test -f dev.deb || curl -sLo dev.deb http://ppa.launchpad.net/zoogie/sdl2-snapshots/ubuntu/pool/main/libs/libsdl2/libsdl2-dev_2.0.3+z4~20140315-8621-1ppa1precise1_amd64.deb
test -f bin.deb || curl -sLo bin.deb http://ppa.launchpad.net/zoogie/sdl2-snapshots/ubuntu/pool/main/libs/libsdl2/libsdl2_2.0.3+z4~20140315-8621-1ppa1precise1_amd64.deb

dpkg-deb -x bin.deb .
dpkg-deb -x dev.deb .

sed -e s,/usr,$DEST,g $DEST/usr/bin/sdl2-config > $DEST/usr/bin/sdl2-config.new
mv $DEST/usr/bin/sdl2-config.new $DEST/usr/bin/sdl2-config
chmod a+x $DEST/usr/bin/sdl2-config

