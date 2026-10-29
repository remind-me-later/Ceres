-- SPDX-License-Identifier: MIT
--
-- The CPU CGB, the Game Boy Color's chip: its blocks wired together, behind
-- the chip's pins. Outside it are the crystal, the cartridge (ROM, RAM and
-- its MBC) and the work RAM on the cartridge bus (with three more lines for
-- its banks), the buttons, the link port, the LCD and the amplifier (see
-- ../board/gameboy.vhd). The VRAM is inside.
--
-- This is the DMG-CPU B's model (branch hdl) changed into the CGB's: the
-- blocks are the DMG's, which the netlist describes, and what the CGB does
-- differently comes from tests, emulators and documents, each tagged where it
-- is (`@hw inferred:`); ../cgb.md lists the differences.
--
-- Inside, the CPU core reaches the other blocks through the chip's address
-- bus (addr) and data bus (data): each block decodes its own addresses from
-- addr, and reads (cpu_rd) or writes (cpu_wr) data. The data bus is
-- precharged high in phase A (external_bus and the CPU core), and keeps its
-- value while nobody drives it.
--
-- @hw die: dmg-schematics, the DMG-CPU B; checked against dmg-sim
-- (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;
use work.revisions.all;

entity cgb_cpu is
	generic (
		BOOTROM  : string;                -- the file with the boot ROM's 2304 bytes
		REVISION : revision_t := CGB_E;

		-- Whether the VRAM starts with zeros (as the boot ROM leaves it)
		-- instead of arbitrary bytes
		VRAM_ZERO : boolean := false;

		-- Without the four sound channels (the registers of the sound
		-- controller stay) the simulation runs about 15 % faster; the
		-- channels' registers read as unconnected, and nothing sounds.
		WITH_CHANNELS : boolean := true
	);
	port (
		-- The crystal, between CK1 and CK2
		ck1 : in  std_ulogic;
		ck2 : out std_ulogic;

		-- /RESET, high once the Game Boy is powered; the test pins, low
		reset_n : in std_ulogic;
		t1, t2  : in std_ulogic;

		-- The cartridge bus
		phi        : out   std_ulogic;  -- the M-cycle clock
		a          : inout std_logic_vector(15 downto 0);
		d          : inout std_logic_vector(7 downto 0);
		rd_n, wr_n : inout std_logic;
		cs_n       : out   std_ulogic;  -- A000 to FDFF
		wram_bank  : out   std_ulogic_vector(2 downto 0);  -- A12 to A14 of the work RAM

		-- The link port
		sin  : inout std_logic;
		sout : out   std_ulogic;
		sck  : inout std_logic;

		-- The buttons: P10 to P13 read them, P14 and P15 select their row
		p10_13   : inout std_logic_vector(3 downto 0);
		p14, p15 : out   std_logic;

		-- The LCD
		cpg, cp, cpl, fr, st, s : out std_ulogic;
		rgb                     : out std_ulogic_vector(14 downto 0);  -- the pixel: red low, blue high

		-- Sound: the cartridge's (VIN), and the right and left outputs, in
		-- volts
		vin      : in  real;
		so1, so2 : out real
	);
end entity;

architecture rtl of cgb_cpu is
	-- The CPU core
	signal read, write, ctl_mreq     : std_ulogic;
	signal mreq, fexx_ffxx           : std_ulogic;
	signal boot_slow                 : std_ulogic := '1';
	signal osc_ena                   : std_ulogic;
	signal clk_ena                   : std_ulogic;
	signal inta                      : std_ulogic_vector(7 downto 0);
	signal acks                      : std_ulogic_vector(4 downto 0);
	signal addr                      : std_logic_vector(15 downto 0);
	signal data                      : std_logic_vector(7 downto 0);

	-- Between the blocks
	signal clk                                                  : std_ulogic;
	signal phase                                                : phase_t;
	signal clk_1mhz, clk_t4, data_phase, exec_phase, pch_phase  : std_ulogic;
	signal cpu_wr_sync, cpu_ext_cs_en                           : std_ulogic;
	signal reset, osc_stable, sys_reset, cpu_sys_reset, reset2  : std_ulogic;
	signal div                                                  : unsigned(15 downto 0);
	signal cpu_rd, cpu_wr, non_vram_mreq                        : std_ulogic;
	signal t1_only, t2_only, test_reset                         : std_ulogic;
	signal ff60                                                 : std_ulogic_vector(1 downto 0);
	signal boot_sel, boot_on                                    : std_ulogic;
	signal cgb_mode, dmg_priority                               : std_ulogic;
	signal clk4, double_speed, switch_wake, ppu_ena                    : std_ulogic;
	signal apu_div                                              : unsigned(15 downto 0);
	signal int_timer, int_serial, int_jp, int_vbl, int_stat     : std_ulogic;
	signal cpu_wakeup                                           : std_ulogic;
	signal sin_drive, sout_test                                 : std_ulogic;
	signal flags                                                : std_ulogic_vector(4 downto 0);
	signal dma_phi                                              : std_ulogic;
	signal dma_run, dma_addr_ext, vram_to_oam                   : std_ulogic;
	signal dma_addr                                             : std_ulogic_vector(15 downto 0);
	signal phase_core                                           : phase_t;
	signal clk_1mhz_core, clk_t4_core, data_phase_core          : std_ulogic;
	signal exec_phase_core, pch_phase_core                      : std_ulogic;
	signal hdma_hold, hdma_bus, hdma_vram_write, ext_dma        : std_ulogic;
	signal hdma_bus_addr, ext_dma_addr                          : std_ulogic_vector(15 downto 0);
	signal hdma_vram_addr                                       : std_ulogic_vector(12 downto 0);
	signal hdma_vram_data                                       : std_ulogic_vector(7 downto 0);
	signal m1                                                   : std_ulogic;
	signal lcdc                                                 : std_ulogic_vector(7 downto 0);
	signal ppu_4mhz, hard_reset, ppu_reset                      : std_ulogic;
	signal wuvu, wosu, hclk, vclk, mode1                        : std_ulogic;
	signal lx                                                   : unsigned(6 downto 0);
	signal ly                                                   : unsigned(7 downto 0);
	signal mode2, mode3, mode0, mode0_early                     : std_ulogic;
	signal line_start, scanning, ceno, stop_scan                : std_ulogic;
	signal scan_index                                           : unsigned(5 downto 0);
	signal start_line, hblank_hdma                              : std_ulogic;
	signal y_match                                              : std_ulogic;
	signal sprite_line_in                                       : std_ulogic_vector(3 downto 0);
	signal save                                                 : std_ulogic_vector(9 downto 0);
	signal x_match                                              : std_ulogic;
	signal render_index                                         : std_logic_vector(5 downto 0);
	signal render_line                                          : std_logic_vector(3 downto 0);
	signal oam_addr, oam_even, oam_odd                          : std_ulogic_vector(7 downto 0);
	signal px                                                   : unsigned(7 downto 0);
	signal rendering, fetch_read, fetch_hold, sprite_loaded     : std_ulogic;
	signal render_ma                                            : std_logic_vector(12 downto 0);
	signal render_bank, pixel_obj                               : std_ulogic;
	signal pixel_palette                                        : std_ulogic_vector(2 downto 0);
	signal pixel_colour                                         : std_ulogic_vector(1 downto 0);
	signal vram_attr                                            : std_ulogic_vector(7 downto 0);
	signal fetching, sprite_read, sprite_mrd                    : std_ulogic;
	signal apu_reset, apu_phi, apu_test                         : std_ulogic;
	signal ajer, ch1_1mhz, ch2_1mhz, ch3_2mhz, ch4_1mhz, khz512 : std_ulogic;
	signal hz512, hz256, hz128, hz128_ch1                       : std_ulogic;
	signal apu_active, apu_dac                                  : std_ulogic_vector(4 downto 1);
	signal ch1_out, ch2_out, ch3_out, ch4_out                   : std_ulogic_vector(3 downto 0);

	-- The VRAM bus inside the chip (the address and the data that the CPU,
	-- the PPU and OAM DMA put on it), and the wires to the VRAM (the DMG's
	-- pins)
	signal vram_addr           : std_logic_vector(13 downto 0);
	signal vram_data           : std_logic_vector(7 downto 0);
	signal ma                  : std_ulogic_vector(13 downto 0);
	signal md                  : std_logic_vector(7 downto 0);
	signal mcs_n, mrd_n, mwr_n : std_logic;

	-- The bus keeper
	signal kept : std_ulogic_vector(7 downto 0) := (others => '1');
begin
	-- The oscillator: an inverter between the crystal's pins.
	-- @hw die: the pads CK1 and CK2
	ck2 <= not ck1;

	-- The CPU core. The chip decodes FE00 to FFFF and the boot ROM's
	-- addresses for its memory request, which is slower than the request:
	-- the request is high for a moment at the start of such accesses,
	-- enough for the latches of the address pins. (Both decodings take
	-- about 10 ns in dmg-sim.)
	-- @hw die: TUNA SYRO, TULO TUTU
	fexx_ffxx <= '1' after 10 ns when addr(15 downto 9) = "1111111" else '0' after 10 ns;
	boot_slow <= boot_sel after 10 ns;

	u_cpu : entity work.sm83
		generic map (REVISION => REVISION)
		port map (
			clk => clk_1mhz_core, exec_phase => exec_phase_core, data_phase => data_phase_core,
			write_phase => clk_t4_core, pch_phase => pch_phase_core, sys_reset => cpu_sys_reset,
			pwron_reset => reset, clk_ready => osc_stable, wake => cpu_wakeup or switch_wake, nmi => '0',
			oe_n => t1_only, internal_access => fexx_ffxx, shadow_access => boot_slow,
			shadow_override => t2_only, int => "000" & flags,
			a => addr, d => data, ctl_mreq => ctl_mreq, mreq => mreq, rd => read, wr => write,
			m1 => m1, halt_n => clk_ena, stop_n => osc_ena, inta => inta
		);
	acks <= inta(4 downto 0);

	u_clocks : entity work.clocks
		generic map (REVISION => REVISION)
		port map (
			ck1 => ck1, reset_pin_n => reset_n, t1 => t1, t2 => t2, phi => phi,
			osc_ena => osc_ena, clk_ena => clk_ena, mreq => mreq, write => write,
			test_reset => test_reset, div_16hz => div(15), double_speed => double_speed,
			ppu_ena => ppu_ena, core_hold => hdma_hold, clk => clk, phase => phase, clk4 => clk4,
			clk_1mhz => clk_1mhz, clk_t4 => clk_t4, data_phase => data_phase,
			exec_phase => exec_phase, pch_phase => pch_phase, cpu_wr_sync => cpu_wr_sync,
			cpu_ext_cs_en => cpu_ext_cs_en,
			phase_core => phase_core, clk_1mhz_core => clk_1mhz_core, clk_t4_core => clk_t4_core,
			data_phase_core => data_phase_core, exec_phase_core => exec_phase_core,
			pch_phase_core => pch_phase_core,
			reset => reset, osc_stable => osc_stable, sys_reset => sys_reset,
			cpu_sys_reset => cpu_sys_reset, reset2 => reset2
		);

	u_divider : entity work.divider
		port map (
			clk => clk, phase => phase, reset => reset, osc_ena => osc_ena, test_fast => ff60(1),
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr, div => div
		);

	u_timer : entity work.timer
		port map (
			clk => clk, phase => phase, clk_t4 => clk_t4, reset2 => reset2, div => div,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr, int_timer => int_timer
		);

	u_interrupts : entity work.interrupts
		port map (
			reset2 => reset2, requests => int_jp & int_serial & int_timer & int_stat & int_vbl,
			acks => acks, addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr, flags => flags
		);

	u_serial : entity work.serial
		port map (
			reset2 => reset2, div => div,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			sin => sin, sout => sout, sck => sck,
			test_pins => ff60(0), sin_drive => sin_drive, sout_test => sout_test,
			int_serial => int_serial
		);

	u_joypad : entity work.joypad
		port map (
			clk => clk, phase => phase, clk_1mhz => clk_1mhz, reset2 => reset2,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			p10_13 => p10_13, p14 => p14, p15 => p15,
			test_pins => ff60(0), sin_drive => sin_drive, sout_test => sout_test,
			int_jp => int_jp, cpu_wakeup => cpu_wakeup
		);

	u_test : entity work.test_mode
		port map (
			t1 => t1, t2 => t2, reset => reset, reset2 => reset2,
			addr => addr, data => data, cpu_wr => cpu_wr,
			t1_only => t1_only, t2_only => t2_only, test_reset => test_reset, ff60 => ff60
		);

	u_boot_rom : entity work.boot_rom
		generic map (
			IMAGE => BOOTROM, REVISION => REVISION
		)
		port map (
			reset2 => reset2, t2_only => t2_only,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			boot_sel => boot_sel, boot_on => boot_on
		);

	u_speed_switch : entity work.speed_switch
		port map (
			ck1 => ck1, reset2 => reset2, cgb_mode => cgb_mode, osc_ena => osc_ena,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			double_speed => double_speed, wake => switch_wake, ppu_ena => ppu_ena
		);

	u_cgb_mode : entity work.cgb_mode
		port map (
			reset2 => reset2, boot_on => boot_on,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			cgb_mode => cgb_mode, dmg_priority => dmg_priority
		);

	u_wram_bank : entity work.wram_bank
		port map (
			reset2 => reset2, cgb_mode => cgb_mode, boot_on => boot_on,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			a12_pin => a(12), bank_pins => wram_bank
		);

	u_high_ram : entity work.high_ram
		port map (
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr
		);

	u_external_bus : entity work.external_bus
		port map (
			clk_t4 => clk_t4_core, exec_phase => exec_phase_core, cpu_wr_sync => cpu_wr_sync,
			cpu_ext_cs_en => cpu_ext_cs_en,
			read => read, write => write, mreq => mreq, boot_sel => boot_sel,
			dma_addr_ext => ext_dma, dma_addr => ext_dma_addr,
			t1_only => t1_only, t2_only => t2_only,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			non_vram_mreq => non_vram_mreq,
			a_pins => a, d_pins => d, rd_n => rd_n, wr_n => wr_n, cs_n => cs_n
		);

	-- HDMA takes the cartridge's bus as OAM DMA does (the CPU core waits).
	ext_dma      <= dma_addr_ext or hdma_bus;
	ext_dma_addr <= hdma_bus_addr when hdma_bus = '1' else dma_addr;

	u_hdma : entity work.hdma
		port map (
			clk => clk, phase => phase, reset2 => reset2, cgb_mode => cgb_mode,
			double_speed => double_speed,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			fetch => m1, running => clk_ena, mode0 => hblank_hdma, lcd_on => lcdc(7),
			hold => hdma_hold, bus_on => hdma_bus, bus_addr => hdma_bus_addr, d_pins => d,
			vram_write => hdma_vram_write, vram_addr => hdma_vram_addr, vram_data => hdma_vram_data
		);

	-- @hw die: UVYT
	dma_phi <= not data_phase;

	u_dma : entity work.dma
		port map (
			dma_phi => dma_phi, hard_reset => reset2,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			dma_run => dma_run, dma_addr => dma_addr, dma_addr_ext => dma_addr_ext,
			vram_to_oam => vram_to_oam
		);

	-- The PPU runs on the 4 MiHz clock, whatever the CPU's speed.
	u_lcd_control : entity work.lcd_control
		port map (
			clk => clk4, reset2 => reset2,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			lcdc => lcdc, ppu_4mhz => ppu_4mhz, hard_reset => hard_reset, ppu_reset => ppu_reset
		);

	u_lcd_timing : entity work.lcd_timing
		generic map (REVISION => REVISION)
		port map (
			clk => clk4, ppu_reset => ppu_reset, hard_reset => hard_reset, lcd_on => lcdc(7), div => div,
			double_speed => double_speed,
			mode3 => mode3, mode0 => mode0, mode0_early => mode0_early,
			scanning => scanning, scan_index => scan_index,
			dma_run => dma_run,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			wuvu => wuvu, wosu => wosu, hclk => hclk, vclk => vclk, lx => lx, ly => ly, mode1 => mode1,
			start_line => start_line, hblank_hdma => hblank_hdma,
			int_vbl => int_vbl, int_stat => int_stat,
			cpl => cpl, cpg => cpg, fr => fr, s => s
		);

	u_oam_scan : entity work.oam_scan
		port map (
			clk => clk4, ppu_reset => ppu_reset, wuvu => wuvu, wosu => wosu, vclk => vclk, ly => ly,
			tall => lcdc(2), dma_run => dma_run, sprite_y => oam_even,
			line_start => line_start, mode2 => mode2, scanning => scanning, ceno => ceno,
			stop_scan => stop_scan, index => scan_index,
			y_match => y_match, sprite_line => sprite_line_in, save => save
		);

	-- The CGB fetches the sprites on the line whatever LCDC bit 1 says (it
	-- only stops drawing them, ../render.vhd): the DMG's fetches need it.
	-- @hw inferred: SameBoy, ceres
	u_sprite_store : entity work.sprite_store
		port map (
			wuvu => wuvu, line_start => line_start, ppu_reset => ppu_reset,
			oam_index => unsigned(oam_addr(7 downto 2)), save => save, sprite_x => oam_odd,
			sprite_line => sprite_line_in,
			px => px, enabled => rendering, rendering => rendering, loaded => sprite_loaded,
			x_match => x_match, index => render_index, line => render_line
		);

	u_oam : entity work.oam
		port map (
			clk => clk4, hard_reset => hard_reset, clk_t4 => clk_t4, dma_phi => dma_phi,
			wuvu => wuvu, wosu => wosu,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			dma_run => dma_run, dma_addr => dma_addr(7 downto 0), dma_addr_ext => dma_addr_ext,
			vram_to_oam => vram_to_oam, d_pins => d, md => vram_data,
			mode2 => mode2, mode3 => mode3, scan_index => scan_index, render_index => render_index,
			fetch_read => fetch_read, fetch_hold => fetch_hold,
			oam_addr => oam_addr, even => oam_even, odd => oam_odd
		);

	u_render : entity work.render
		generic map (REVISION => REVISION)
		port map (
			clk => clk4, ppu_reset => ppu_reset, hard_reset => hard_reset, hclk => hclk,
			mode1 => mode1, ly => ly, lcdc => lcdc, cgb_mode => cgb_mode, dmg_priority => dmg_priority,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			line_start => line_start, stop_scan => stop_scan, ceno => ceno, x_match => x_match,
			sprite_line => render_line, tile => oam_even, attributes => oam_odd, sprite_index => render_index,
			md => vram_data, md_attr => vram_attr,
			mode3 => mode3, mode0 => mode0, mode0_early => mode0_early, px => px,
			rendering => rendering, fetch_read => fetch_read, fetch_hold => fetch_hold,
			sprite_loaded => sprite_loaded,
			ma => render_ma, bank => render_bank, fetching => fetching, sprite_read => sprite_read,
			sprite_mrd => sprite_mrd, cp => cp, st => st,
			pixel_obj => pixel_obj, pixel_palette => pixel_palette, pixel_colour => pixel_colour
		);

	u_colour_palettes : entity work.colour_palettes
		port map (
			reset2 => reset2, cgb_mode => cgb_mode, boot_on => boot_on,
			clk4 => clk4, double_speed => double_speed, start_line => start_line, mode3 => mode3,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			pixel_obj => pixel_obj, pixel_palette => pixel_palette, pixel_colour => pixel_colour,
			rgb => rgb
		);

	u_vram_bus : entity work.vram_bus
		generic map (REVISION => REVISION)
		port map (
			hard_reset => hard_reset, reset2 => reset2, cgb_mode => cgb_mode,
			clk_t4 => clk_t4_core, cpu_wr_sync => cpu_wr_sync,
			cpu_ext_cs_en => cpu_ext_cs_en, t1_only => t1_only,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, write => write, non_vram_mreq => non_vram_mreq,
			vram_to_oam => vram_to_oam, dma_addr => dma_addr(12 downto 0),
			hdma_write => hdma_vram_write, hdma_addr => hdma_vram_addr, hdma_data => hdma_vram_data,
			clk4 => clk4, double_speed => double_speed, start_line => start_line,
			mode3 => mode3, fetching => fetching, sprite_read => sprite_read, sprite_mrd => sprite_mrd,
			render_ma => render_ma, render_bank => render_bank, ma => vram_addr, md => vram_data,
			ma_pins => ma(12 downto 0), ma13 => ma(13), md_pins => md, mcs_n => mcs_n, mrd_n => mrd_n, mwr_n => mwr_n
		);

	u_video_ram : entity work.video_ram
		generic map (
			ZERO => VRAM_ZERO
		)
		port map (
			ma => ma, md => md, mcs_n => mcs_n, mrd_n => mrd_n, mwr_n => mwr_n, md_attr => vram_attr
		);

	-- The DMG's board keeps the VRAM's data and control lines; so do these
	-- wires.
	md_keeper : entity work.keeper generic map (WIDTH => 8) port map (lines => md);
	controls_keeper : entity work.keeper
		generic map (
			WIDTH => 3
		)
		port map (
			lines(0) => mrd_n, lines(1) => mwr_n, lines(2) => mcs_n
		);

	-- The APU's dividers run on the 4 MiHz clock, and its frame sequencer
	-- takes the divider's bit that ticks at 512 Hz in either speed (DIV bit
	-- 4, or 5 in double speed); its registers are the CPU's.
	-- @hw inferred: Pan Docs ("Audio": APU ticks do not change with the
	-- speed; "Audio details": DIV bit 5 in double speed)
	apu_div <= shift_right(div, 1) when double_speed = '1' else div;

	u_apu_control : entity work.apu_control
		port map (
			clk => clk4, reset2 => reset2, data_phase => data_phase, clk_t4 => clk_t4, div => apu_div,
			t1_only => t1_only,
			addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			apu_reset => apu_reset, apu_phi => apu_phi, test => apu_test,
			ajer => ajer, ch1_1mhz => ch1_1mhz, ch2_1mhz => ch2_1mhz, ch3_2mhz => ch3_2mhz,
			ch4_1mhz => ch4_1mhz, khz512 => khz512,
			hz512 => hz512, hz256 => hz256, hz128 => hz128, hz128_ch1 => hz128_ch1,
			active => apu_active, dac_on => apu_dac,
			ch1 => ch1_out, ch2 => ch2_out, ch3 => ch3_out, ch4 => ch4_out, vin => vin,
			so1 => so1, so2 => so2
		);

	channels : if WITH_CHANNELS generate
		u_apu_ch2 : entity work.apu_ch2
			port map (
				apu_reset => apu_reset, apu_phi => apu_phi, clk_t4 => clk_t4, ch2_1mhz => ch2_1mhz,
				hz256 => hz256, hz128 => hz128, hz512 => hz512, test => apu_test,
				addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
				active => apu_active(2), dac_on => apu_dac(2), output => ch2_out
			);

		u_apu_ch1 : entity work.apu_ch1
			port map (
				apu_reset => apu_reset, apu_phi => apu_phi, clk_t4 => clk_t4, ajer => ajer, ch1_1mhz => ch1_1mhz,
				hz256 => hz256, hz128 => hz128, hz128_ch1 => hz128_ch1, hz512 => hz512, test => apu_test,
				addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
				active => apu_active(1), dac_on => apu_dac(1), output => ch1_out
			);

		u_apu_ch3 : entity work.apu_ch3
			port map (
				clk => clk4, apu_reset => apu_reset, apu_phi => apu_phi, clk_t4 => clk_t4, pch_phase => pch_phase,
				ajer => ajer, ch3_2mhz => ch3_2mhz, hz256 => hz256, test => apu_test,
				addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
				active => apu_active(3), dac_on => apu_dac(3), output => ch3_out
			);

		u_apu_ch4 : entity work.apu_ch4
			port map (
				apu_reset => apu_reset, apu_phi => apu_phi, clk_t4 => clk_t4, ch4_1mhz => ch4_1mhz, khz512 => khz512,
				hz256 => hz256, hz128 => hz128, hz512 => hz512, test => apu_test,
				addr => addr, data => data, data_out => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
				active => apu_active(4), dac_on => apu_dac(4), output => ch4_out
			);
	end generate;

	no_channels : if not WITH_CHANNELS generate
		apu_active <= (others => '0');
		apu_dac    <= (others => '0');
		ch1_out    <= (others => '0');
		ch2_out    <= (others => '0');
		ch3_out    <= (others => '0');
		ch4_out    <= (others => '0');
	end generate;

	-- The bus keeper holds the last value driven onto the data bus.
	keeper : process (data)
	begin
		for i in data'range loop
			if data(i) = '0' or data(i) = '1' then
				kept(i) <= data(i);
			end if;
		end loop;
	end process;

	keep : for i in data'range generate
		data(i) <= 'H' when kept(i) = '1' else 'L';
	end generate;
end architecture;
