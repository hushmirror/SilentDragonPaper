#!/bin/bash
# This script depends on a docker image already being built
# To build it, 
# cd docker
# docker build --tag rustbuild:latest .

POSITIONAL=()
while [[ $# -gt 0 ]]
do
key="$1"

case $key in
    -v|--version)
    APP_VERSION="$2"
    shift # past argument
    shift # past value
    ;;
    *)    # unknown option
    POSITIONAL+=("$1") # save it in an array for later
    shift # past argument
    ;;
esac
done
set -- "${POSITIONAL[@]}" # restore positional parameters

if [ -z $APP_VERSION ]; then echo "APP_VERSION is not set"; exit 1; fi

# Clean everything first
cargo clean

# Compile for mac directly
cargo build --release 

# macOS
rm -rf target/macOS-hushpaperwallet-v$APP_VERSION
mkdir -p target/macOS-hushpaperwallet-v$APP_VERSION
cp target/release/hushpaperwallet target/macOS-hushpaperwallet-v$APP_VERSION/

# For Windows and Linux, build via docker
docker run --rm -v $(pwd)/..:/opt/hushpaperwallet rustbuild:latest bash -c "cd /opt/hushpaperwallet/cli && cargo build --release && cargo build --release --target x86_64-pc-windows-gnu && cargo build --release --target aarch64-unknown-linux-gnu"

# Now sign and zip the binaries
gpg --batch --output target/macOS-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig target/macOS-hushpaperwallet-v$APP_VERSION/hushpaperwallet 
cd target
cd macOS-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet > sha256sum.txt
cd ..
zip -r macOS-hushpaperwallet-v$APP_VERSION.zip macOS-hushpaperwallet-v$APP_VERSION 
cd ..


#Linux
rm -rf target/linux-hushpaperwallet-v$APP_VERSION
mkdir -p target/linux-hushpaperwallet-v$APP_VERSION
cp target/release/hushpaperwallet target/linux-hushpaperwallet-v$APP_VERSION/
gpg --batch --output target/linux-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig target/linux-hushpaperwallet-v$APP_VERSION/hushpaperwallet
cd target
cd linux-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet > sha256sum.txt
cd ..
zip -r linux-hushpaperwallet-v$APP_VERSION.zip linux-hushpaperwallet-v$APP_VERSION 
cd ..


#Windows
rm -rf target/Windows-hushpaperwallet-v$APP_VERSION
mkdir -p target/Windows-hushpaperwallet-v$APP_VERSION
cp target/x86_64-pc-windows-gnu/release/hushpaperwallet.exe target/Windows-hushpaperwallet-v$APP_VERSION/
gpg --batch --output target/Windows-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig target/Windows-hushpaperwallet-v$APP_VERSION/hushpaperwallet.exe
cd target
cd Windows-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet.exe > sha256sum.txt
cd ..
zip -r Windows-hushpaperwallet-v$APP_VERSION.zip Windows-hushpaperwallet-v$APP_VERSION 
cd ..


# aarch64 (armv8)
rm -rf target/aarch64-hushpaperwallet-v$APP_VERSION
mkdir -p target/aarch64-hushpaperwallet-v$APP_VERSION
cp target/aarch64-unknown-linux-gnu/release/hushpaperwallet target/aarch64-hushpaperwallet-v$APP_VERSION/
gpg --batch --output target/aarch64-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig target/aarch64-hushpaperwallet-v$APP_VERSION/hushpaperwallet
cd target
cd aarch64-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet > sha256sum.txt
cd ..
zip -r aarch64-hushpaperwallet-v$APP_VERSION.zip aarch64-hushpaperwallet-v$APP_VERSION 
cd ..

