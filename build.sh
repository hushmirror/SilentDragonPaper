#!/bin/bash
# Copyright (c) 2016-2021 The Hush developers
# Distributed under the GPLv3 software license, see the accompanying
# file COPYING or https://www.gnu.org/licenses/gpl-3.0.en.html

set -eu -o pipefail

# TODO: find elite Rust coders to update our shit
# to work on modern versions of rustc, lulz

PREFIX=rust-1.48.0-x86_64-unknown-linux-gnu
FILE=$PREFIX.tar.gz

if [ ! -f "$FILE" ]; then
    wget https://static.rust-lang.org/dist/$FILE
fi

#TODO: verify SHA256
# 950420a35b2dd9091f1b93a9ccd5abc026ca7112e667f246b1deb79204e2038b  rust-1.48.0-x86_64-unknown-linux-gnu.tar.gz

tar zxvpf $FILE
mkdir -p build
cd $PREFIX
./install.sh --prefix=$(pwd)/../build

cd ../cli
PATH=$(pwd)/../build/bin/:$PATH
echo PATH=$PATH
cargo --version
rustc --version
../build/bin/cargo build --verbose --release
