.PHONY: all test clean

all:
	$(MAKE) -C bootroms
	cd emulator && cargo build --release

clean:
	$(MAKE) -C bootroms clean
	cd emulator && cargo clean
