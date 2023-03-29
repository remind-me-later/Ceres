.PHONY: all test clean

all:
	$(MAKE) -C gb-bootroms
	cd emulator && cargo build --release

clean:
	$(MAKE) -C gb-bootroms clean
	cd emulator && cargo clean
