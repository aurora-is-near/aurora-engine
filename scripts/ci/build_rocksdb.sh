#!/usr/bin/env bash

ROCKSDB_VER=v8.1.1
INSTALL_PATH=/root/rocksdb
LIB_PATH=$INSTALL_PATH/lib/librocksdb.a

if [[ ! -f $LIB_PATH ]]; then
  apt -y install cmake libgflags-dev libsnappy-dev libbz2-dev liblz4-dev libzstd-dev
  git clone --branch $ROCKSDB_VER https://github.com/facebook/rocksdb
  cd rocksdb && mkdir build && cd build
  cmake -DCMAKE_BUILD_TYPE=Release -DWITH_ZLIB=1 -DWITH_SNAPPY=1 -DWITH_LZ4=1 -DWITH_ZSTD=1 -DCMAKE_INSTALL_PREFIX=$INSTALL_PATH ..
  make -j32 install
  cache-util save rocksdb:$INSTALL_PATH
  cd $HOME
fi
