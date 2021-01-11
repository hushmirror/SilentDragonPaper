#!/bin/bash
# Copyright (c) 2016-2021 The Hush developers
# Distributed under the GPLv3 software license, see the accompanying
# file COPYING or https://www.gnu.org/licenses/gpl-3.0.en.html

set -eu -o pipefail

# TODO: find elite Rust coders to update our shit
# to work on modern versions of rustc, lulz

PREFIX=rust-1.48.0-x86_64-unknown-linux-gnu
FILE=$PREFIX.tar.gz
SHA=950420a35b2dd9091f1b93a9ccd5abc026ca7112e667f246b1deb79204e2038b

if [ ! -f "$FILE" ]; then
    wget https://static.rust-lang.org/dist/$FILE
fi

# Verify SHA256 of rust
echo "$SHA  $FILE" | shasum -a 256 --check
# TWO SPACES or sadness sometimes:
# https://unix.stackexchange.com/questions/139891/why-does-verifying-sha256-checksum-with-sha256sum-fail-on-debian-and-work-on-u
echo "$SHA  $FILE" | shasum -a 256 --check --status
if [ $? -ne 0 ]; then
    FOUNDSHA=$(shasum -a 256 $FILE)
    echo "SHA256 mismatch on $FILE!"
    echo "$FOUNDSHA did not match $SHA . Aborting..."
    exit 1
fi

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
