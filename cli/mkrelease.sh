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
rm -rf target/macOS-silentdragonpaper-v$APP_VERSION
mkdir -p target/macOS-silentdragonpaper-v$APP_VERSION
cp target/release/silentdragonpaper target/macOS-silentdragonpaper-v$APP_VERSION/

# For Windows and Linux, build via docker
docker run --rm -v $(pwd)/..:/opt/silentdragonpaper rustbuild:latest bash -c "cd /opt/silentdragonpaper/cli && cargo build --release && cargo build --release --target x86_64-pc-windows-gnu && cargo build --release --target aarch64-unknown-linux-gnu"

# Now sign and zip the binaries
gpg --batch --output target/macOS-silentdragonpaper-v$APP_VERSION/silentdragonpaper.sig --detach-sig target/macOS-silentdragonpaper-v$APP_VERSION/silentdragonpaper 
cd target
cd macOS-silentdragonpaper-v$APP_VERSION
gsha256sum silentdragonpaper > sha256sum.txt
cd ..
zip -r macOS-silentdragonpaper-v$APP_VERSION.zip macOS-silentdragonpaper-v$APP_VERSION 
cd ..


#Linux
rm -rf target/linux-silentdragonpaper-v$APP_VERSION
mkdir -p target/linux-silentdragonpaper-v$APP_VERSION
cp target/release/silentdragonpaper target/linux-silentdragonpaper-v$APP_VERSION/
gpg --batch --output target/linux-silentdragonpaper-v$APP_VERSION/silentdragonpaper.sig --detach-sig target/linux-silentdragonpaper-v$APP_VERSION/silentdragonpaper
cd target
cd linux-silentdragonpaper-v$APP_VERSION
gsha256sum silentdragonpaper > sha256sum.txt
cd ..
zip -r linux-silentdragonpaper-v$APP_VERSION.zip linux-silentdragonpaper-v$APP_VERSION 
cd ..


#Windows
rm -rf target/Windows-silentdragonpaper-v$APP_VERSION
mkdir -p target/Windows-silentdragonpaper-v$APP_VERSION
cp target/x86_64-pc-windows-gnu/release/silentdragonpaper.exe target/Windows-silentdragonpaper-v$APP_VERSION/
gpg --batch --output target/Windows-silentdragonpaper-v$APP_VERSION/silentdragonpaper.sig --detach-sig target/Windows-silentdragonpaper-v$APP_VERSION/silentdragonpaper.exe
cd target
cd Windows-silentdragonpaper-v$APP_VERSION
gsha256sum silentdragonpaper.exe > sha256sum.txt
cd ..
zip -r Windows-silentdragonpaper-v$APP_VERSION.zip Windows-silentdragonpaper-v$APP_VERSION 
cd ..


# aarch64 (armv8)
rm -rf target/aarch64-silentdragonpaper-v$APP_VERSION
mkdir -p target/aarch64-silentdragonpaper-v$APP_VERSION
cp target/aarch64-unknown-linux-gnu/release/silentdragonpaper target/aarch64-silentdragonpaper-v$APP_VERSION/
gpg --batch --output target/aarch64-silentdragonpaper-v$APP_VERSION/silentdragonpaper.sig --detach-sig target/aarch64-silentdragonpaper-v$APP_VERSION/silentdragonpaper
cd target
cd aarch64-silentdragonpaper-v$APP_VERSION
gsha256sum silentdragonpaper > sha256sum.txt
cd ..
zip -r aarch64-silentdragonpaper-v$APP_VERSION.zip aarch64-silentdragonpaper-v$APP_VERSION 
cd ..

