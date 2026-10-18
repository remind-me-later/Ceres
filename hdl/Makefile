# The gate-level oracle: dmg-sim, the simulation of the DMG-CPU B netlist,
# on Icarus Verilog, with our probes compiled in (see README.md).
#
#   make            build dmg-sim from its submodule, with the probes
#   make run        simulate SECS seconds after the reset, trace the pins
#   make smoke      33 ms from power-on with dmg-sim's quickboot ROM: the
#                   pin trace must have the hash in smoke.sha256
#
# Variables:
#   SECS     simulated seconds after the chip's power-on reset (31.2 ms)
#   ROM      cartridge ROM (none, MBC1 or MBC5 from its header; "" for none)
#   BOOTROM  boot ROM image
#   TRACE    file for the changes of the pins ("" for none)

SECS    ?= 0.01
ROM     ?=
BOOTROM ?= ../external/gb-bootroms/bin/dmg.bin
TRACE   ?= trace.txt

BUILD   := build
DMG_SIM := ../external/reference-implementations/dmg-sim
SIM     := $(BUILD)/dmg-sim
VVP     := $(SIM)/dmg_cpu_b_gameboy.vvp
PROBES  := oracle/probe_pins.sv oracle/probe_results.sv

QUICKBOOT := $(DMG_SIM)/boot/quickboot.bin

abs = $(abspath $(1))

.PHONY: all run smoke clean

all: $(VVP)

# dmg-sim is copied from its submodule, which stays clean. Icarus 13 has no
# string method tolower, which dmg-sim only uses for event files.
$(VVP): $(PROBES)
	rm -rf $(SIM)
	mkdir -p $(BUILD)
	cp -r $(DMG_SIM) $(SIM)
	sed -i 's|cmd = cmd.tolower();|// cmd = cmd.tolower();|' $(SIM)/dmg_cpu_b_gameboy.sv
	$(MAKE) -C $(SIM) dmg_cpu_b_gameboy.vvp \
		IVERILOG_FLAGS="-g2012 -gspecify -f timescale.f -pfileline=1 -gsupported-assertions $(call abs,$(PROBES))"

run: $(VVP)
	vvp -N $(VVP) -none +BOOTROM=$(call abs,$(BOOTROM)) $(if $(ROM),+ROM=$(call abs,$(ROM))) \
		+SECS=$(SECS) $(if $(TRACE),+PINS_FILE=$(call abs,$(TRACE)))

# The changes at one time are sorted, so that the hash does not depend on
# the order Icarus runs the probes in.
smoke:
	$(MAKE) run SECS=0.0018 BOOTROM=$(QUICKBOOT) TRACE=$(BUILD)/smoke.txt
	LC_ALL=C sort $(BUILD)/smoke.txt | sha256sum -c smoke.sha256

clean:
	rm -rf $(BUILD)
