.PHONY: all test clean

all:
	$(MAKE) -C ceres_core/bootroms
	cargo build --release

clean:
	$(MAKE) -C ceres_core/bootroms clean
	cargo clean
