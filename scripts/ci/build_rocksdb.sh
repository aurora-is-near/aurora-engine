#!/usr/bin/env bash

ROCKSDB_VER=v8.1.1
INSTALL_PATH=$HOME/.rocksdb
LIB_PATH=$INSTALL_PATH/lib/librocksdb.a

if [[ ! -f $LIB_PATH ]]; then
  sudo apt-get -y install build-essential cmake libgflags-dev libsnappy-dev libbz2-dev liblz4-dev libzstd-dev
  git clone --branch $ROCKSDB_VER https://github.com/facebook/rocksdb
  cd rocksdb && mkdir build && cd build || exit
  cmake -DCMAKE_BUILD_TYPE=Release -DWITH_ZLIB=1 -DWITH_SNAPPY=1 -DWITH_LZ4=1 -DWITH_ZSTD=1 -DCMAKE_INSTALL_PREFIX=$INSTALL_PATH ..
  make -j 64 install
  cd "$HOME" || exit
fi
