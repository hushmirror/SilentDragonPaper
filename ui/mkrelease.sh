#!/bin/bash

# Accept the variables as command line arguments as well
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


if [ -z $APP_VERSION ]; then
    echo "APP_VERSION is not set. Please set it to the current release version of the app";
    exit 1;
fi

# This should be set as an environment variable
if [ -z $QT_PATH ]; then 
    echo "QT_PATH is not set. Please set it to the base directory of Qt"; 
    exit 1; 
fi
QT_STATIC=$QT_PATH/clang_64/bin

# Build for MacOS first

# Clean
echo -n "Cleaning..............."
$QT_STATIC/qmake papersapling.pro CONFIG+=release >/dev/null
make distclean >/dev/null 2>&1
rm -rf    artifacts/macOS-hushpaperwallet-v$APP_VERSION
mkdir -p  artifacts/macOS-hushpaperwallet-v$APP_VERSION
echo "[OK]"

echo -n "Testing................"
cd ../lib
if ! cargo test --release; then
    echo "[Test Failed]"
    exit 1;
fi
cd ../ui

echo -n "Configuring............"
# Build
$QT_STATIC/qmake papersapling.pro CONFIG+=release >/dev/null
APP_BUILD_DATE=$(date +%F)
echo "#define APP_VERSION \"$APP_VERSION\"" > src/version.h
echo "#define APP_BUILD_DATE \"$APP_BUILD_DATE\"" >> src/version.h

echo "[OK]"


echo -n "Building..............."
make -j4 >/dev/null
echo "[OK]"

#Qt deploy
echo -n "Deploying.............."
$QT_STATIC/macdeployqt hushpaperwalletui.app 
cp -r hushpaperwalletui.app artifacts/macOS-hushpaperwallet-v$APP_VERSION/
echo "[OK]"

# Run inside docker container
docker run --rm -v ${PWD}/..:/opt/hushpaperwallet hushwallet/compileenv:v0.8 bash -c "cd /opt/hushpaperwallet/ui && ./mkdockerwinlinux.sh -v $APP_VERSION"

# Move to build the cli
cd ../cli

# Clean everything first
cargo clean
echo "pub fn version() -> &'static str { &\"$APP_VERSION\" }" > src/version.rs

# Compile for mac directly and copy it over
cargo build --release 
cp target/release/hushpaperwallet ../ui/artifacts/macOS-hushpaperwallet-v$APP_VERSION/

# For Windows and Linux, build via docker
docker run --rm -v $(pwd)/..:/opt/hushpaperwallet rust/hushpaperwallet:v0.3 bash -c "cd /opt/hushpaperwallet/cli && cargo build --release  && cargo build --release --target x86_64-pc-windows-gnu && cargo build --release --target aarch64-unknown-linux-gnu && cargo build --release --target armv7-unknown-linux-gnueabihf"

# Come back and package everything
cd ../ui

# Now sign and zip the binaries
#macOS
# binary is already copied above
gpg --batch --output artifacts/macOS-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig artifacts/macOS-hushpaperwallet-v$APP_VERSION/hushpaperwallet 
#gpg --batch --output artifacts/macOS-hushpaperwallet-v$APP_VERSION/hushpaperwallet.app.sig --detach-sig artifacts/macOS-hushpaperwallet-v$APP_VERSION/hushpaperwallet.app 
cd artifacts
cd macOS-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet > sha256sum.txt
cd ..
zip -r macOS-hushpaperwallet-v$APP_VERSION.zip macOS-hushpaperwallet-v$APP_VERSION 
cd ..


#Linux
cp ../cli/target/release/hushpaperwallet artifacts/linux-hushpaperwallet-v$APP_VERSION/
gpg --batch --output artifacts/linux-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig artifacts/linux-hushpaperwallet-v$APP_VERSION/hushpaperwallet
gpg --batch --output artifacts/linux-hushpaperwallet-v$APP_VERSION/hushpaperwalletui.sig --detach-sig artifacts/linux-hushpaperwallet-v$APP_VERSION/hushpaperwalletui
cd artifacts
cd linux-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet hushpaperwalletui > sha256sum.txt
cd ..
zip -r linux-hushpaperwallet-v$APP_VERSION.zip linux-hushpaperwallet-v$APP_VERSION 
cd ..


#Windows
cp ../cli/target/x86_64-pc-windows-gnu/release/hushpaperwallet.exe artifacts/Windows-hushpaperwallet-v$APP_VERSION/
gpg --batch --output artifacts/Windows-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig artifacts/Windows-hushpaperwallet-v$APP_VERSION/hushpaperwallet.exe
gpg --batch --output artifacts/Windows-hushpaperwallet-v$APP_VERSION/hushpaperwalletui.sig --detach-sig artifacts/Windows-hushpaperwallet-v$APP_VERSION/hushpaperwalletui.exe
cd artifacts
cd Windows-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet.exe hushpaperwalletui.exe > sha256sum.txt
cd ..
zip -r Windows-hushpaperwallet-v$APP_VERSION.zip Windows-hushpaperwallet-v$APP_VERSION 
cd ..


# aarch64 (armv8)
rm -rf artifacts/aarch64-hushpaperwallet-v$APP_VERSION
mkdir -p artifacts/aarch64-hushpaperwallet-v$APP_VERSION
cp ../cli/target/aarch64-unknown-linux-gnu/release/hushpaperwallet artifacts/aarch64-hushpaperwallet-v$APP_VERSION/
gpg --batch --output artifacts/aarch64-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig artifacts/aarch64-hushpaperwallet-v$APP_VERSION/hushpaperwallet
cd artifacts
cd aarch64-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet > sha256sum.txt
cd ..
zip -r aarch64-hushpaperwallet-v$APP_VERSION.zip aarch64-hushpaperwallet-v$APP_VERSION 
cd ..


# ARMv7
rm -rf artifacts/armv7-hushpaperwallet-v$APP_VERSION
mkdir -p artifacts/armv7-hushpaperwallet-v$APP_VERSION
cp ../cli/target/armv7-unknown-linux-gnueabihf/release/hushpaperwallet artifacts/armv7-hushpaperwallet-v$APP_VERSION/
gpg --batch --output artifacts/armv7-hushpaperwallet-v$APP_VERSION/hushpaperwallet.sig --detach-sig artifacts/armv7-hushpaperwallet-v$APP_VERSION/hushpaperwallet
cd artifacts
cd armv7-hushpaperwallet-v$APP_VERSION
gsha256sum hushpaperwallet > sha256sum.txt
cd ..
zip -r armv7-hushpaperwallet-v$APP_VERSION.zip armv7-hushpaperwallet-v$APP_VERSION 
cd ..

