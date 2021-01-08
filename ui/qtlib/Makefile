ifeq ($(shell uname),Darwin)
    EXT := dylib
else
    EXT := a
endif

all: release

winrelease: target/x86_64-pc-windows-gnu/release/silentdragonpaper.lib

target/x86_64-pc-windows-gnu/release/silentdragonpaper.lib: src/lib.rs Cargo.toml
	cargo build --lib --release --target x86_64-pc-windows-gnu

release: target/release/silentdragonpaper.$(EXT)
debug: target/debug/silentdragonpaper.$(EXT) 

target/release/silentdragonpaper.$(EXT): src/lib.rs Cargo.toml
	cargo build --lib --release

target/debug/silentdragonpaper.$(EXT): src/lib.rs Cargo.toml
	cargo build --lib

clean:
	rm -rf target
