# Gate-level VHDL model of the Game Boy (see README.md).
#
#   make                 generate the VHDL from the submodules and analyse it
#   make run             simulate TIME of a Game Boy, write the pin trace
#   make SIM=ghdl run    the same with GHDL instead of nvc
#   make ref-run         simulate the same with dmg-sim (Icarus Verilog)
#   make diff            compare the pin traces of run and ref-run
#   make smoke           33 ms from power-on with dmg-sim's quickboot: the pin
#                        trace must have the hash in smoke.sha256, which was
#                        checked against dmg-sim (update it only after a diff)
#
# Variables:
#   TIME     simulated time, e.g. "40 ms" (the chip leaves reset after 31.2 ms)
#   ROM      cartridge ROM: none, MBC1 or MBC5 ("" for no cartridge)
#   TRACE    file for the changes of the pins
#   BOOTROM  boot ROM image
#   REF_SECS how long dmg-sim simulates after the chip's reset (31.2 ms)
#   REF_TRACE file for the pins of dmg-sim

SIM     ?= nvc
TIME    ?= 40 ms
ROM     ?=
TRACE   ?= trace.txt
BOOTROM ?= ../external/gb-bootroms/bin/dmg.bin
REF_SECS  ?= 0.008
REF_TRACE ?= ref.txt

BUILD := build
GEN   := $(BUILD)/gen
WORK  := $(BUILD)/$(SIM)

GENERATED := $(GEN)/cells.vhd $(GEN)/sm83.vhd $(GEN)/dmg_cpu_b.vhd
SOURCES   := support/verilog.vhd gate/timing.vhd $(GEN)/cells.vhd gate/cells.vhd \
             $(GEN)/sm83.vhd $(GEN)/dmg_cpu_b.vhd tb/gameboy_tb.vhd
GENERATOR := $(wildcard ../ceres-hdlgen/src/*.rs ../ceres-hdlgen/src/*/*.rs)

NVC  := nvc -M 4g --std=2008
GHDL := ghdl

# Absolute paths, since the simulators run in $(WORK).
abs = $(abspath $(1))
rom_generic = $(if $(ROM),-gROM_FILE=$(call abs,$(ROM)))

DMG_SIM := ../external/reference-implementations/dmg-sim
REF     := $(BUILD)/ref

QUICKBOOT := $(DMG_SIM)/boot/quickboot.bin

.PHONY: all gen run ref ref-run diff smoke clean

all: $(WORK)/analysed

gen: $(GENERATED)

$(GENERATED) &: $(GENERATOR)
	cargo run --release -p ceres-hdlgen --bin ceres-hdlgen -- --out $(GEN)

$(BUILD)/nvc/analysed: $(SOURCES)
	mkdir -p $(BUILD)/nvc
	cd $(BUILD)/nvc && $(NVC) -a $(call abs,$(SOURCES))
	touch $@

$(BUILD)/ghdl/analysed: $(SOURCES)
	mkdir -p $(BUILD)/ghdl
	cd $(BUILD)/ghdl && $(GHDL) -a --std=08 $(call abs,$(SOURCES))
	touch $@

run: $(WORK)/analysed
	ln -sf $(call abs,$(BOOTROM)) $(WORK)/bootrom.bin
ifeq ($(SIM),nvc)
	cd $(WORK) && $(NVC) -e --jit gameboy_tb -gSIM_TIME='$(TIME)' $(rom_generic) \
		-gTRACE_FILE=$(call abs,$(TRACE)) -r
else
	cd $(WORK) && $(GHDL) --elab-run --std=08 gameboy_tb -gSIM_TIME='$(TIME)' $(rom_generic) \
		-gTRACE_FILE=$(call abs,$(TRACE))
endif

# dmg-sim, copied from its submodule, with a probe of the pins. Icarus 13
# has no string method tolower, which dmg-sim only uses for event files.
ref: $(REF)/dmg-sim/dmg_cpu_b_gameboy.vvp

$(REF)/dmg-sim/dmg_cpu_b_gameboy.vvp: ref/probe_pins.sv
	rm -rf $(REF)/dmg-sim
	mkdir -p $(REF)
	cp -r $(DMG_SIM) $(REF)/dmg-sim
	sed -i 's|cmd = cmd.tolower();|// cmd = cmd.tolower();|' $(REF)/dmg-sim/dmg_cpu_b_gameboy.sv
	$(MAKE) -C $(REF)/dmg-sim dmg_cpu_b_gameboy.vvp \
		IVERILOG_FLAGS="-g2012 -gspecify -f timescale.f -pfileline=1 -gsupported-assertions $(call abs,ref/probe_pins.sv)"

ref-run: ref
	$(MAKE) -C $(REF)/dmg-sim sim-gameboy DUMP= SECS=$(REF_SECS) \
		BOOTROM=$(call abs,$(BOOTROM)) ROM=$(if $(ROM),$(call abs,$(ROM)))
	cp $(REF)/dmg-sim/pins.trace $(REF_TRACE)

# The first 100 us differ while unknown values settle after power-up.
diff:
	cargo run --release -p ceres-hdlgen --bin hdl-trace-diff -- $(REF_TRACE) $(TRACE) --from 100000000

smoke:
	$(MAKE) run TIME="33 ms" BOOTROM=$(QUICKBOOT) TRACE=$(BUILD)/smoke.txt
	cd $(BUILD) && sha256sum -c ../smoke.sha256

clean:
	rm -rf $(BUILD)
