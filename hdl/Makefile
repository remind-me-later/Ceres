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

# The generator writes the cells (cells.txt) and the chip, one file per
# block of the schematics (chip.txt), and lists them in the order they can
# be analysed.
GENERATED := $(GEN)/cells.txt $(GEN)/chip.txt
GENERATOR := $(wildcard ../ceres-hdlgen/src/*.rs ../ceres-hdlgen/src/*/*.rs)

# The hand-written files, in the order they are analysed (a unit after the
# units it uses).
SUPPORT := support/verilog.vhd gate/keeper.vhd gate/timing.vhd
CELLS   := $(addprefix gate/cells/,bus_bit.vhd memory.vhd simplified_sram.vhd \
             high_ram.vhd oam.vhd wave_ram.vhd boot_rom.vhd audio.vhd)
TB      := $(addprefix tb/,util.vhd video_ram.vhd work_ram.vhd cartridge.vhd \
             serial_receiver.vhd pin_trace.vhd oam_conflict.vhd mooneye_result.vhd \
             gbmicrotest_result.vhd gameboy_tb.vhd)

# All of them: the generated cells after the support files, the chip after
# all the cells, the testbench last. The lists are read when a recipe runs,
# since the generator writes them.
generated = $$(sed 's|^|$(abspath $(GEN))/|' $(abspath $(GEN))/$(1))
SOURCES = $(call abs,$(SUPPORT)) $(call generated,cells.txt) $(call abs,$(CELLS)) \
          $(call generated,chip.txt) $(call abs,$(TB))

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

$(BUILD)/nvc/analysed: $(SUPPORT) $(GENERATED) $(CELLS) $(TB)
	mkdir -p $(BUILD)/nvc
	cd $(BUILD)/nvc && $(NVC) -a $(SOURCES)
	touch $@

$(BUILD)/ghdl/analysed: $(SUPPORT) $(GENERATED) $(CELLS) $(TB)
	mkdir -p $(BUILD)/ghdl
	cd $(BUILD)/ghdl && $(GHDL) -a --std=08 $(SOURCES)
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
