-- SPDX-License-Identifier: MIT
--
-- Checks the model against dmg-sim, phase by phase. dmg-sim's samples
-- (ORACLE, see ../oracle/probe_phases.sv) give the inputs: the crystal and
-- the pins, with what the chips on the cartridge bus and the VRAM bus
-- answer. At the end of every phase, the model's signals must have
-- dmg-sim's values. The blocks of the model, the CPU core among them, drive
-- each other, so that what passes between them within a phase (such as the
-- STAT write glitch) counts.
--
-- Within a phase, the inputs follow dmg-sim's timing roughly: the chips on
-- the cartridge bus and the VRAM bus answer 10 ns after the edge. The
-- internal data bus is precharged high in phase A (external_bus and the CPU
-- core), and keeps its value while nobody drives it.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;
use work.oracle.all;
use work.sprite_slots.all;
use work.sm83_controls.all;

entity chip_tb is
	generic (
		ORACLE  : string;
		BOOTROM : string
	);
end entity;

architecture check of chip_tb is
	-- Pins
	signal ck1, reset_pin_n, t1, t2 : std_ulogic := '0';
	signal phi                      : std_ulogic;
	signal a_pins                   : std_logic_vector(15 downto 0);
	signal d_pins                   : std_logic_vector(7 downto 0);
	signal rd_n, wr_n               : std_logic;
	signal cs_n                     : std_ulogic;
	signal sin, sck                 : std_logic;
	signal sout                     : std_ulogic;
	signal p10_13                   : std_logic_vector(3 downto 0);
	signal p14, p15                 : std_logic;
	signal cpl, cpg, fr, s          : std_ulogic;

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

	-- The VRAM chip's pins
	signal ma_pins             : std_ulogic_vector(12 downto 0);
	signal md_pins             : std_logic_vector(7 downto 0);
	signal mcs_n, mrd_n, mwr_n : std_logic;
	signal cp, st              : std_ulogic;
	signal ld                  : std_ulogic_vector(1 downto 0);

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
	signal boot_sel                                             : std_ulogic;
	signal int_timer, int_serial, int_jp, int_vbl, int_stat     : std_ulogic;
	signal cpu_wakeup                                           : std_ulogic;
	signal sin_drive, sout_test                                 : std_ulogic;
	signal flags                                                : std_ulogic_vector(4 downto 0);
	signal dma_phi                                              : std_ulogic;
	signal dma_run, dma_addr_ext, vram_to_oam                   : std_ulogic;
	signal dma_addr                                             : std_ulogic_vector(15 downto 0);
	signal lcdc                                                 : std_ulogic_vector(7 downto 0);
	signal ppu_4mhz, hard_reset, ppu_reset                      : std_ulogic;
	signal wuvu, wosu, hclk, vclk, mode1                        : std_ulogic;
	signal lx                                                   : unsigned(6 downto 0);
	signal ly                                                   : unsigned(7 downto 0);
	signal mode2, mode3, mode0                                  : std_ulogic;
	signal line_start, scanning, ceno, stop_scan                : std_ulogic;
	signal scan_index                                           : unsigned(5 downto 0);
	signal y_match                                              : std_ulogic;
	signal sprite_line_in                                       : std_ulogic_vector(3 downto 0);
	signal save                                                 : std_ulogic_vector(9 downto 0);
	signal x_match                                              : std_ulogic;
	signal render_index                                         : std_logic_vector(5 downto 0);
	signal render_line                                          : std_logic_vector(3 downto 0);
	signal oam_addr, oam_even, oam_odd                          : std_ulogic_vector(7 downto 0);
	signal px                                                   : unsigned(7 downto 0);
	signal rendering, fetch_read, fetch_hold, sprite_loaded     : std_ulogic;
	signal render_ma, ma                                        : std_logic_vector(12 downto 0);
	signal fetching, sprite_read, sprite_mrd                    : std_ulogic;
	signal md                                                   : std_logic_vector(7 downto 0);
	signal apu_reset, apu_phi, apu_test                         : std_ulogic;
	signal ajer, ch1_1mhz, ch2_1mhz, ch3_2mhz, ch4_1mhz, khz512 : std_ulogic;
	signal hz512, hz256, hz128, hz128_ch1                       : std_ulogic;
	signal apu_active, apu_dac                                  : std_ulogic_vector(4 downto 1);
	signal ch1_out, ch2_out, ch3_out, ch4_out                   : std_ulogic_vector(3 downto 0);
	signal so1, so2                                             : real;

	-- The bus keeper
	signal kept : std_ulogic_vector(7 downto 0) := (others => '1');

	shared variable dmg : oracle_t;
begin
	-- The CPU core. The chip decodes FE00 to FFFF and the boot ROM's
	-- addresses for its memory request, which is slower than the request:
	-- the request is high for a moment at the start of such accesses,
	-- enough for the latches of the address pins. (Both decodings take
	-- about 10 ns in dmg-sim.)
	-- @hw die: the chip's TUNA SYRO, TULO TUTU
	fexx_ffxx <= '1' after 10 ns when addr(15 downto 9) = "1111111" else '0' after 10 ns;
	boot_slow <= boot_sel after 10 ns;

	u_cpu : entity work.sm83
		port map (
			clk => clk_1mhz, exec_phase => exec_phase, data_phase => data_phase,
			write_phase => clk_t4, pch_phase => pch_phase, sys_reset => cpu_sys_reset,
			pwron_reset => reset, clk_ready => osc_stable, wake => cpu_wakeup, nmi => '0',
			oe_n => t1_only, internal_access => fexx_ffxx, shadow_access => boot_slow,
			shadow_override => t2_only, int => "000" & flags,
			a => addr, d => data, ctl_mreq => ctl_mreq, mreq => mreq, rd => read, wr => write,
			m1 => open, halt_n => clk_ena, stop_n => osc_ena, inta => inta
		);
	acks <= inta(4 downto 0);

	u_clocks : entity work.clocks
		port map (
			ck1 => ck1, reset_pin_n => reset_pin_n, t1 => t1, t2 => t2, phi => phi,
			osc_ena => osc_ena, clk_ena => clk_ena, mreq => mreq, write => write,
			test_reset => test_reset, div_16hz => div(15),
			clk => clk, phase => phase,
			clk_1mhz => clk_1mhz, clk_t4 => clk_t4, data_phase => data_phase,
			exec_phase => exec_phase, pch_phase => pch_phase, cpu_wr_sync => cpu_wr_sync,
			cpu_ext_cs_en => cpu_ext_cs_en,
			reset => reset, osc_stable => osc_stable, sys_reset => sys_reset,
			cpu_sys_reset => cpu_sys_reset, reset2 => reset2
		);

	u_divider : entity work.divider
		port map (
			clk => clk, phase => phase, reset => reset, osc_ena => osc_ena, test_fast => ff60(1),
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr, div => div
		);

	u_timer : entity work.timer
		port map (
			clk => clk, phase => phase, clk_t4 => clk_t4, reset2 => reset2, div => div,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr, int_timer => int_timer
		);

	u_interrupts : entity work.interrupts
		port map (
			reset2 => reset2, requests => int_jp & int_serial & int_timer & int_stat & int_vbl,
			acks => acks, addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr, flags => flags
		);

	u_serial : entity work.serial
		port map (
			reset2 => reset2, div => div,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			sin => sin, sout => sout, sck => sck,
			test_pins => ff60(0), sin_drive => sin_drive, sout_test => sout_test,
			int_serial => int_serial
		);

	u_joypad : entity work.joypad
		port map (
			clk => clk, phase => phase, clk_1mhz => clk_1mhz, reset2 => reset2,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
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
			IMAGE => BOOTROM
		)
		port map (
			reset2 => reset2, t2_only => t2_only,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr, boot_sel => boot_sel
		);

	u_high_ram : entity work.high_ram
		port map (
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr
		);

	u_external_bus : entity work.external_bus
		port map (
			clk_t4 => clk_t4, exec_phase => exec_phase, cpu_wr_sync => cpu_wr_sync,
			cpu_ext_cs_en => cpu_ext_cs_en,
			read => read, write => write, mreq => mreq, boot_sel => boot_sel,
			dma_addr_ext => dma_addr_ext, dma_addr => dma_addr,
			t1_only => t1_only, t2_only => t2_only,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			non_vram_mreq => non_vram_mreq,
			a_pins => a_pins, d_pins => d_pins, rd_n => rd_n, wr_n => wr_n, cs_n => cs_n
		);

	-- @hw die: UVYT
	dma_phi <= not data_phase;

	u_dma : entity work.dma
		port map (
			dma_phi => dma_phi, hard_reset => reset2,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			dma_run => dma_run, dma_addr => dma_addr, dma_addr_ext => dma_addr_ext,
			vram_to_oam => vram_to_oam
		);

	u_lcd_control : entity work.lcd_control
		port map (
			clk => clk, reset2 => reset2,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			lcdc => lcdc, ppu_4mhz => ppu_4mhz, hard_reset => hard_reset, ppu_reset => ppu_reset
		);

	u_lcd_timing : entity work.lcd_timing
		port map (
			clk => clk, ppu_reset => ppu_reset, hard_reset => hard_reset, lcd_on => lcdc(7), div => div,
			mode2 => mode2, mode3 => mode3, mode0 => mode0,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			wuvu => wuvu, wosu => wosu, hclk => hclk, vclk => vclk, lx => lx, ly => ly, mode1 => mode1,
			int_vbl => int_vbl, int_stat => int_stat,
			cpl => cpl, cpg => cpg, fr => fr, s => s
		);

	u_oam_scan : entity work.oam_scan
		port map (
			clk => clk, ppu_reset => ppu_reset, wuvu => wuvu, wosu => wosu, vclk => vclk, ly => ly,
			tall => lcdc(2), dma_run => dma_run, sprite_y => oam_even,
			line_start => line_start, mode2 => mode2, scanning => scanning, ceno => ceno,
			stop_scan => stop_scan, index => scan_index,
			y_match => y_match, sprite_line => sprite_line_in, save => save
		);

	u_sprite_store : entity work.sprite_store
		port map (
			wuvu => wuvu, line_start => line_start, ppu_reset => ppu_reset,
			oam_index => unsigned(oam_addr(7 downto 2)), save => save, sprite_x => oam_odd,
			sprite_line => sprite_line_in,
			px => px, enabled => rendering and lcdc(1), rendering => rendering, loaded => sprite_loaded,
			x_match => x_match, index => render_index, line => render_line
		);

	u_oam : entity work.oam
		port map (
			clk => clk, hard_reset => hard_reset, clk_t4 => clk_t4, dma_phi => dma_phi,
			wuvu => wuvu, wosu => wosu,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			dma_run => dma_run, dma_addr => dma_addr(7 downto 0), dma_addr_ext => dma_addr_ext,
			vram_to_oam => vram_to_oam, d_pins => d_pins, md => md,
			mode2 => mode2, mode3 => mode3, scan_index => scan_index, render_index => render_index,
			fetch_read => fetch_read, fetch_hold => fetch_hold,
			oam_addr => oam_addr, even => oam_even, odd => oam_odd
		);

	u_render : entity work.render
		port map (
			clk => clk, ppu_reset => ppu_reset, hard_reset => hard_reset, hclk => hclk,
			mode1 => mode1, ly => ly, lcdc => lcdc,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			line_start => line_start, stop_scan => stop_scan, ceno => ceno, x_match => x_match,
			sprite_line => render_line, tile => oam_even, attributes => oam_odd, md => md,
			mode3 => mode3, mode0 => mode0, px => px,
			rendering => rendering, fetch_read => fetch_read, fetch_hold => fetch_hold,
			sprite_loaded => sprite_loaded,
			ma => render_ma, fetching => fetching, sprite_read => sprite_read, sprite_mrd => sprite_mrd,
			cp => cp, st => st, ld => ld
		);

	u_vram_bus : entity work.vram_bus
		port map (
			hard_reset => hard_reset, clk_t4 => clk_t4, cpu_wr_sync => cpu_wr_sync,
			cpu_ext_cs_en => cpu_ext_cs_en, t1_only => t1_only,
			addr => addr, data => data, cpu_rd => cpu_rd, write => write, non_vram_mreq => non_vram_mreq,
			vram_to_oam => vram_to_oam, dma_addr => dma_addr(12 downto 0),
			mode3 => mode3, fetching => fetching, sprite_read => sprite_read, sprite_mrd => sprite_mrd,
			render_ma => render_ma, ma => ma, md => md,
			ma_pins => ma_pins, md_pins => md_pins, mcs_n => mcs_n, mrd_n => mrd_n, mwr_n => mwr_n
		);

	u_apu_control : entity work.apu_control
		port map (
			clk => clk, reset2 => reset2, data_phase => data_phase, clk_t4 => clk_t4, div => div,
			t1_only => t1_only,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			apu_reset => apu_reset, apu_phi => apu_phi, test => apu_test,
			ajer => ajer, ch1_1mhz => ch1_1mhz, ch2_1mhz => ch2_1mhz, ch3_2mhz => ch3_2mhz,
			ch4_1mhz => ch4_1mhz, khz512 => khz512,
			hz512 => hz512, hz256 => hz256, hz128 => hz128, hz128_ch1 => hz128_ch1,
			active => apu_active, dac_on => apu_dac,
			ch1 => ch1_out, ch2 => ch2_out, ch3 => ch3_out, ch4 => ch4_out, vin => 2.2,
			so1 => so1, so2 => so2
		);

	u_apu_ch2 : entity work.apu_ch2
		port map (
			apu_reset => apu_reset, apu_phi => apu_phi, clk_t4 => clk_t4, ch2_1mhz => ch2_1mhz,
			hz256 => hz256, hz128 => hz128, hz512 => hz512, test => apu_test,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			active => apu_active(2), dac_on => apu_dac(2), output => ch2_out
		);

	u_apu_ch1 : entity work.apu_ch1
		port map (
			apu_reset => apu_reset, apu_phi => apu_phi, clk_t4 => clk_t4, ajer => ajer, ch1_1mhz => ch1_1mhz,
			hz256 => hz256, hz128 => hz128, hz128_ch1 => hz128_ch1, hz512 => hz512, test => apu_test,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			active => apu_active(1), dac_on => apu_dac(1), output => ch1_out
		);

	u_apu_ch3 : entity work.apu_ch3
		port map (
			clk => clk, apu_reset => apu_reset, apu_phi => apu_phi, clk_t4 => clk_t4, pch_phase => pch_phase,
			ajer => ajer, ch3_2mhz => ch3_2mhz, hz256 => hz256, test => apu_test,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			active => apu_active(3), dac_on => apu_dac(3), output => ch3_out
		);

	u_apu_ch4 : entity work.apu_ch4
		port map (
			apu_reset => apu_reset, apu_phi => apu_phi, clk_t4 => clk_t4, ch4_1mhz => ch4_1mhz, khz512 => khz512,
			hz256 => hz256, hz128 => hz128, hz512 => hz512, test => apu_test,
			addr => addr, data => data, cpu_rd => cpu_rd, cpu_wr => cpu_wr,
			active => apu_active(4), dac_on => apu_dac(4), output => ch4_out
		);

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

	run : process
		variable lcd_was_on   : boolean := false;
		variable mode3_was_on : boolean := false;
	begin
		dmg.open_file(ORACLE);
		while dmg.next_phase loop
			-- The edge that starts the phase.
			ck1 <= dmg.bit_value("ck1");
			if dmg.phase > 0 then
				wait for 1 ns;
			end if;

			reset_pin_n <= dmg.bit_value("reset_n");
			t1          <= dmg.bit_value("t1");
			t2          <= dmg.bit_value("t2");

			-- The cartridge, the work RAM and the VRAM chip answer the chip's
			-- pins 10 ns after the edge, when they do not see the chip drive
			-- their data.
			if dmg.phase > 0 then
				wait for 9 ns;
			end if;
			if dmg.bit_value("pins_out") = '1' then
				d_pins <= (others => 'Z');
			else
				d_pins <= dmg.vector_value("d_pins");
			end if;
			if dmg.bit_value("md_out") = '1' then
				md_pins <= (others => 'Z');
			else
				md_pins <= dmg.vector_value("md_pins");
			end if;

			if dmg.phase > 0 then
				wait for T_phase - 11 ns;
			else
				wait for T_phase - 1 ns;
			end if;

			-- Clocks and resets
			dmg.check("phase", image(phase));
			dmg.check("clk", image(clk));
			dmg.check("phi", image(phi));
			dmg.check("clk_1mhz", image(clk_1mhz));
			dmg.check("clk_t4", image(clk_t4));
			dmg.check("data_phase", image(data_phase));
			dmg.check("exec_phase", image(exec_phase));
			dmg.check("pch_phase", image(pch_phase));
			dmg.check("cpu_wr_sync", image(cpu_wr_sync));
			dmg.check("cpu_ext_cs_en", image(cpu_ext_cs_en));
			dmg.check("reset", image(reset));
			dmg.check("osc_stable", image(osc_stable));
			dmg.check("sys_reset", image(sys_reset));
			dmg.check("cpu_sys_reset", image(cpu_sys_reset));
			dmg.check("reset2", image(reset2));
			dmg.check("div", image(std_ulogic_vector(div)));

			-- The CPU core
			dmg.check("mreq", image(mreq));
			dmg.check("ctl_mreq", image(ctl_mreq));
			dmg.check("read", image(read));
			dmg.check("write", image(write));
			dmg.check("osc_ena", image(osc_ena));
			dmg.check("clk_ena", image(clk_ena));
			dmg.check("int_acks", image(acks));
			dmg.check("addr", image(std_ulogic_vector(addr)));
			dmg.check("cpu_opcode", image(<< signal .chip_tb.u_cpu.opcode : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_mcyc", image(<< signal .chip_tb.u_cpu.mcyc : std_ulogic_vector(2 downto 0) >>));
			-- In dmg-sim, the interrupt logic's dynamic nodes start
			-- undefined: int_entry and int_pending are 1 for the first
			-- M-cycle, while the reset holds the core.
			if dmg.phase > 8 then
				dmg.check("cpu_state", image(<< signal .chip_tb.u_cpu.table_cb : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.in_intr : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.halt : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.stop : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.int_entry : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.ime : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.u_sequencer.ime_state : std_ulogic >>));
				dmg.check("cpu_control", image(<< signal .chip_tb.u_cpu.c : controls_t >>.fetch)
					& image(<< signal .chip_tb.u_cpu.c : controls_t >>.mread)
					& image(<< signal .chip_tb.u_cpu.c : controls_t >>.mwrite)
					& image(<< signal .chip_tb.u_cpu.sel_reg_w : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.cond_fail : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.int_take : std_ulogic >>)
					& image(<< signal .chip_tb.u_cpu.int_pending : std_ulogic >>));
			end if;
			dmg.check("cpu_a", image(<< signal .chip_tb.u_cpu.u_registers.a : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_flags", image(<< signal .chip_tb.u_cpu.flags : std_ulogic_vector(7 downto 4) >>));
			dmg.check("cpu_bc", image(<< signal .chip_tb.u_cpu.u_registers.b : std_ulogic_vector(7 downto 0) >>)
				& image(<< signal .chip_tb.u_cpu.u_registers.c_reg : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_de", image(<< signal .chip_tb.u_cpu.u_registers.d : std_ulogic_vector(7 downto 0) >>)
				& image(<< signal .chip_tb.u_cpu.u_registers.e : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_hl", image(<< signal .chip_tb.u_cpu.u_registers.h : std_ulogic_vector(7 downto 0) >>)
				& image(<< signal .chip_tb.u_cpu.u_registers.l : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_sp", image(<< signal .chip_tb.u_cpu.u_registers.sph : std_ulogic_vector(7 downto 0) >>)
				& image(<< signal .chip_tb.u_cpu.u_registers.spl : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_pc", image(<< signal .chip_tb.u_cpu.u_registers.pch : std_ulogic_vector(7 downto 0) >>)
				& image(<< signal .chip_tb.u_cpu.u_registers.pcl : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_wz", image(<< signal .chip_tb.u_cpu.u_registers.w : std_ulogic_vector(7 downto 0) >>)
				& image(<< signal .chip_tb.u_cpu.u_registers.z : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_ie", image(<< signal .chip_tb.u_cpu.u_interrupts.ie : std_ulogic_vector(7 downto 0) >>));
			dmg.check_bus("cpu_db", image(std_ulogic_vector(<< signal .chip_tb.u_cpu.db : std_logic_vector(7 downto 0) >>)));
			dmg.check("cpu_res", image(<< signal .chip_tb.u_cpu.res : std_ulogic_vector(7 downto 0) >>));
			dmg.check("cpu_idu", image(<< signal .chip_tb.u_cpu.idu_out : std_ulogic_vector(15 downto 0) >>));

			-- The timer and the interrupt flags
			dmg.check("tima", image(<< signal .chip_tb.u_timer.tima : std_ulogic_vector(7 downto 0) >>));
			dmg.check("tma", image(<< signal .chip_tb.u_timer.tma : std_ulogic_vector(7 downto 0) >>));
			dmg.check("tac", image(<< signal .chip_tb.u_timer.tac : std_ulogic_vector(2 downto 0) >>));
			dmg.check("tima_bit7_before", image(<< signal .chip_tb.u_timer.bit7_before : std_ulogic >>));
			dmg.check("int_timer", image(int_timer));
			dmg.check("int_flags", image(flags));

			-- The serial port, the joypad and test mode
			dmg.check("sb", image(<< signal .chip_tb.u_serial.sb : std_ulogic_vector(7 downto 0) >>));
			dmg.check("sc", image(<< signal .chip_tb.u_serial.transfer : std_ulogic >>)
			              & image(<< signal .chip_tb.u_serial.internal_clock : std_ulogic >>));
			dmg.check("serial_count", image(std_ulogic_vector(<< signal .chip_tb.u_serial.count : unsigned(3 downto 0) >>)));
			dmg.check("serial_half", image(<< signal .chip_tb.u_serial.half : std_ulogic >>));
			dmg.check("ser_out", image(<< signal .chip_tb.u_serial.ser_out : std_ulogic >>));
			dmg.check("sck", image(sck));
			dmg.check("sin", image(sin));
			dmg.check("sout", image(sout));
			dmg.check("p1", image(<< signal .chip_tb.u_joypad.p1 : std_ulogic_vector(7 downto 0) >>));
			dmg.check("p10_13", image(std_ulogic_vector(p10_13)));
			dmg.check("p14", image(p14));
			dmg.check("p15", image(p15));
			dmg.check("joypad_history", image(<< signal .chip_tb.u_joypad.history : std_ulogic_vector(3 downto 0) >>));
			dmg.check("int_jp", image(int_jp));
			dmg.check("cpu_wakeup", image(cpu_wakeup));
			dmg.check("t1_only", image(t1_only));
			dmg.check("t2_only", image(t2_only));
			dmg.check("test_reset", image(test_reset));
			dmg.check("ff60", image(ff60));

			-- The boot ROM
			dmg.check("boot_unmapped", image(<< signal .chip_tb.u_boot_rom.unmapped : std_ulogic >>));
			dmg.check("boot_sel", image(boot_sel));

			-- The cartridge bus
			dmg.check("cpu_rd", image(cpu_rd));
			dmg.check("cpu_wr", image(cpu_wr));
			dmg.check("non_vram_mreq", image(non_vram_mreq));
			dmg.check("latched_addr", image(<< signal .chip_tb.u_external_bus.latched_addr : std_ulogic_vector(14 downto 0) >>));
			dmg.check("latched_data", image(<< signal .chip_tb.u_external_bus.latched_data : std_ulogic_vector(7 downto 0) >>));
			dmg.check("pins_out", image(<< signal .chip_tb.u_external_bus.pins_out : std_ulogic >>));
			dmg.check("a_pins", image(std_ulogic_vector(a_pins)));
			dmg.check("d_pins", image(std_ulogic_vector(d_pins)));
			dmg.check("rd_n", image(rd_n));
			dmg.check("wr_n", image(wr_n));
			dmg.check("cs_n", image(cs_n));

			-- OAM DMA
			dmg.check("dma_phi", image(dma_phi));
			dmg.check("dma_requested", image(<< signal .chip_tb.u_dma.requested : std_ulogic >>));
			dmg.check("dma_start", image(<< signal .chip_tb.u_dma.start_2 : std_ulogic >>)
			                     & image(<< signal .chip_tb.u_dma.start_1 : std_ulogic >>));
			dmg.check("dma_enabled", image(<< signal .chip_tb.u_dma.enabled : std_ulogic >>));
			dmg.check("dma_done", image(<< signal .chip_tb.u_dma.done : std_ulogic >>));
			dmg.check("dma_run", image(dma_run));
			dmg.check("dma_addr", image(dma_addr));
			dmg.check("dma_addr_ext", image(dma_addr_ext));
			dmg.check("vram_to_oam", image(vram_to_oam));

			-- The PPU: LCDC, and the timing of lines and frames
			dmg.check("lcdc", image(lcdc));
			dmg.check("ppu_reset", image(ppu_reset));
			dmg.check("wuvu", image(wuvu));
			dmg.check("vena", image(<< signal .chip_tb.u_lcd_timing.vena : std_ulogic >>));
			dmg.check("wosu", image(wosu));
			dmg.check("lx", image(std_ulogic_vector(lx)));
			dmg.check("vclk", image(vclk));
			dmg.check("nype", image(<< signal .chip_tb.u_lcd_timing.nype : std_ulogic >>));
			dmg.check("mode1", image(mode1));
			dmg.check("ly", image(std_ulogic_vector(ly)));
			dmg.check("line_153", image(<< signal .chip_tb.u_lcd_timing.line_153 : std_ulogic >>));
			dmg.check("first_line", image(<< signal .chip_tb.u_lcd_timing.first_line : std_ulogic >>));
			dmg.check("frame", image(<< signal .chip_tb.u_lcd_timing.frame : std_ulogic >>));
			dmg.check("line_parity", image(<< signal .chip_tb.u_lcd_timing.line_parity : std_ulogic >>));
			dmg.check("cpg_lx", image(<< signal .chip_tb.u_lcd_timing.cpg_lx : std_ulogic >>));
			dmg.check("lyc", image(<< signal .chip_tb.u_lcd_timing.lyc : std_ulogic_vector(7 downto 0) >>));
			dmg.check("lyc_match", image(<< signal .chip_tb.u_lcd_timing.lyc_match : std_ulogic >>));
			dmg.check("stat_enable", image(<< signal .chip_tb.u_lcd_timing.stat_enable : std_ulogic_vector(6 downto 3) >>));
			dmg.check("int_vbl", image(int_vbl));
			dmg.check("int_stat", image(int_stat));
			dmg.check("cpl", image(cpl));
			dmg.check("cpg", image(cpg));
			dmg.check("fr", image(fr));
			dmg.check("s", image(s));

			-- The PPU's rendering, once the LCD has been on, and the fetcher's
			-- once mode 3 has started: before, their counters run from the
			-- unknown state of power-on.
			if dmg.bit_value("ppu_reset") = '0' then
				lcd_was_on := true;
			end if;
			if dmg.bit_value("mode3") = '1' then
				mode3_was_on := true;
			end if;
			if lcd_was_on then
			dmg.check("mode2", image(mode2));
			dmg.check("mode3", image(mode3));
			dmg.check("mode0", image(mode0));
			dmg.check("px", image(std_ulogic_vector(px)));
			dmg.check("mode_state", image(mode2) & image(mode3) & image(mode0)
				& image(<< signal .chip_tb.u_render.px9 : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.voga : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.stop_mode3 : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.clkpipe : std_ulogic >>) & image(x_match));
			dmg.check("scan_index", image(std_ulogic_vector(scan_index)));
			dmg.check("scan_state", image(<< signal .chip_tb.u_oam_scan.start_scan : std_ulogic >>)
				& image(<< signal .chip_tb.u_oam_scan.anel : std_ulogic >>) & image(scanning) & image(ceno)
				& image(<< signal .chip_tb.u_oam_scan.byba : std_ulogic >>)
				& image(<< signal .chip_tb.u_oam_scan.doba : std_ulogic >>) & image(stop_scan));
			dmg.check("sprite_count", image(std_ulogic_vector(<< signal .chip_tb.u_oam_scan.count : unsigned(3 downto 0) >>)));
			dmg.check("sprite_save", image(<< signal .chip_tb.u_oam_scan.not_saving : std_ulogic >>)
				& image(<< signal .chip_tb.u_oam_scan.save_enable : std_ulogic >>) & image(y_match));
			if mode3_was_on then
			dmg.check("fetch", image(<< signal .chip_tb.u_render.fetch_count : unsigned(2 downto 0) >>(0))
				& image(<< signal .chip_tb.u_render.fetch_count : unsigned(2 downto 0) >>(1))
				& image(<< signal .chip_tb.u_render.fetch_count : unsigned(2 downto 0) >>(2))
				& image(<< signal .chip_tb.u_render.lyzu : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.lovy : std_ulogic >>) & image(fetching)
				& image(<< signal .chip_tb.u_render.nyka : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.pory : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.pygo : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.started : std_ulogic >>));
			end if;
			dmg.check("window_state", image(<< signal .chip_tb.u_render.nopa : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.nunu : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.in_window : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.pyco : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.sovy : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.wy_latch : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.sary : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.wy_match : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.wxy_match : std_ulogic >>));
			dmg.check("fine", image(<< signal .chip_tb.u_render.rene : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.ryfa : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.discard : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.puxa : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.nyze : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.paho : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.fine : unsigned(2 downto 0) >>(1))
				& image(<< signal .chip_tb.u_render.fine : unsigned(2 downto 0) >>(2))
				& image(<< signal .chip_tb.u_render.fine : unsigned(2 downto 0) >>(0)));
			dmg.check("sprite_fetch", image(<< signal .chip_tb.u_render.sprite_step : unsigned(2 downto 0) >>(0))
				& image(<< signal .chip_tb.u_render.sprite_step : unsigned(2 downto 0) >>(1))
				& image(<< signal .chip_tb.u_render.sprite_step : unsigned(2 downto 0) >>(2))
				& image(<< signal .chip_tb.u_render.sobu : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.suda : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.busy : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.tobu : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.vonu : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.seba : std_ulogic >>)
				& image(<< signal .chip_tb.u_render.tyfo : std_ulogic >>));
			dmg.check("scy", image(<< signal .chip_tb.u_render.scy : std_ulogic_vector(7 downto 0) >>));
			dmg.check("scx", image(<< signal .chip_tb.u_render.scx : std_ulogic_vector(7 downto 0) >>));
			dmg.check("wy", image(<< signal .chip_tb.u_render.wy : std_ulogic_vector(7 downto 0) >>));
			dmg.check("wx", image(<< signal .chip_tb.u_render.wx : std_ulogic_vector(7 downto 0) >>));
			dmg.check("bgp", image(<< signal .chip_tb.u_render.bgp : std_ulogic_vector(7 downto 0) >>));
			dmg.check("obp0", image(<< signal .chip_tb.u_render.obp0 : std_ulogic_vector(7 downto 0) >>));
			dmg.check("obp1", image(<< signal .chip_tb.u_render.obp1 : std_ulogic_vector(7 downto 0) >>));
			dmg.check("win_y", image(std_ulogic_vector(<< signal .chip_tb.u_render.win_y : unsigned(7 downto 0) >>)));
			dmg.check("win_x", image(std_ulogic_vector(<< signal .chip_tb.u_render.win_x : unsigned(4 downto 0) >>)));
			dmg.check("bg_tile", image(<< signal .chip_tb.u_render.bg_tile : std_ulogic_vector(7 downto 0) >>));
			dmg.check("bg_low", image(<< signal .chip_tb.u_render.bg_low : std_ulogic_vector(7 downto 0) >>));
			dmg.check("bg_shift_a", image(<< signal .chip_tb.u_render.bg_shift_a : std_ulogic_vector(7 downto 0) >>));
			dmg.check("bg_shift_b", image(<< signal .chip_tb.u_render.bg_shift_b : std_ulogic_vector(7 downto 0) >>));
			dmg.check("sp_low_a", image(<< signal .chip_tb.u_render.sp_low_a : std_ulogic_vector(7 downto 0) >>));
			dmg.check("sp_low_b", image(<< signal .chip_tb.u_render.sp_low_b : std_ulogic_vector(7 downto 0) >>));
			dmg.check("sp_shift_a", image(<< signal .chip_tb.u_render.sp_a : std_ulogic_vector(7 downto 0) >>));
			dmg.check("sp_shift_b", image(<< signal .chip_tb.u_render.sp_b : std_ulogic_vector(7 downto 0) >>));
			dmg.check("sp_shift_pal", image(<< signal .chip_tb.u_render.sp_palette : std_ulogic_vector(7 downto 0) >>));
			dmg.check("sp_shift_prio", image(<< signal .chip_tb.u_render.sp_priority : std_ulogic_vector(7 downto 0) >>));
			dmg.check("oam_addr", image(oam_addr));
			dmg.check("oam_latch_b", image(<< signal .chip_tb.u_oam.first_even : std_ulogic_vector(7 downto 0) >>));
			dmg.check("oam_latch_a", image(<< signal .chip_tb.u_oam.first_odd : std_ulogic_vector(7 downto 0) >>));
			dmg.check("oam_latch_y", image(oam_even));
			dmg.check("oam_latch_x", image(oam_odd));
			dmg.check("ma", image(std_ulogic_vector(ma)));
			dmg.check("md", image(std_ulogic_vector(md)));
			dmg.check("ma_pins", image(ma_pins));
			dmg.check("md_pins", image(std_ulogic_vector(md_pins)));
			dmg.check("mcs_n", image(mcs_n));
			dmg.check("mrd_n", image(mrd_n));
			dmg.check("mwr_n", image(mwr_n));
			for i in 0 to 9 loop
				dmg.check("sprite_x" & integer'image(i), image(<< signal .chip_tb.u_sprite_store.xs : x_t >>(i)));
				dmg.check("sprite_line" & integer'image(i), image(<< signal .chip_tb.u_sprite_store.lines : line_t >>(i)));
				dmg.check("sprite_index" & integer'image(i), image(<< signal .chip_tb.u_sprite_store.indexes : index_t >>(i)));
			end loop;
			dmg.check("cp", image(cp));
			dmg.check("st", image(st));
			dmg.check("ld", image(ld));
			end if;

			-- The APU
			dmg.check("apu_reset", image(apu_reset));
			dmg.check("apu_power", image(<< signal .chip_tb.u_apu_control.power : std_ulogic >>));
			dmg.check("apu_test", image(apu_test) & image(<< signal .chip_tb.u_apu_control.test_bit5 : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_control.baza : std_ulogic >>));
			dmg.check("nr50", image(<< signal .chip_tb.u_apu_control.nr50 : std_ulogic_vector(7 downto 0) >>));
			dmg.check("nr51", image(<< signal .chip_tb.u_apu_control.nr51 : std_ulogic_vector(7 downto 0) >>));
			dmg.check("apu_clocks", image(ajer) & image(ch1_1mhz) & image(<< signal .chip_tb.u_apu_control.atep : std_ulogic >>)
				& image(ch2_1mhz) & image(ch3_2mhz) & image(<< signal .chip_tb.u_apu_control.atyk : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_control.avok : std_ulogic >>) & image(khz512));
			dmg.check("sequencer", image(<< signal .chip_tb.u_apu_control.bara : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_control.caru : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_control.bylu : std_ulogic >>));
			dmg.check("nr21", image(<< signal .chip_tb.u_apu_ch2.duty : std_ulogic_vector(1 downto 0) >>));
			dmg.check("nr22", image(<< signal .chip_tb.u_apu_ch2.envelope : std_ulogic_vector(7 downto 0) >>));
			dmg.check("ch2_frequency", image(<< signal .chip_tb.u_apu_ch2.frequency : std_ulogic_vector(10 downto 0) >>));
			dmg.check("ch2_length_on", image(<< signal .chip_tb.u_apu_ch2.length_on : std_ulogic >>));
			dmg.check("ch2_trigger", image(<< signal .chip_tb.u_apu_ch2.trigger : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.start : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.triggered : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.restart : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.restarted : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.running : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.frozen : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.active_i : std_ulogic >>));
			dmg.check("ch2_counter", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch2.counter : unsigned(10 downto 0) >>)));
			dmg.check("ch2_duty", image(<< signal .chip_tb.u_apu_ch2.reload : std_ulogic >>)
				& image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch2.step : unsigned(2 downto 0) >>))
				& image(<< signal .chip_tb.u_apu_ch2.wave : std_ulogic >>));
			dmg.check("ch2_length", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch2.length : unsigned(5 downto 0) >>)));
			dmg.check("ch2_expired", image(<< signal .chip_tb.u_apu_ch2.expired : std_ulogic >>));
			dmg.check("ch2_envelope", image(<< signal .chip_tb.u_apu_ch2.hz64 : std_ulogic >>)
				& image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch2.period : unsigned(2 downto 0) >>))
				& image(<< signal .chip_tb.u_apu_ch2.tick : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.at_limit : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch2.env_stopped : std_ulogic >>));
			dmg.check("ch2_volume", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch2.volume : unsigned(3 downto 0) >>)));
			dmg.check("ch2_out", image(ch2_out));

			dmg.check("apu_active", image(apu_active));
			dmg.check("apu_dac", image(apu_dac));
			dmg.check("nr10", image(<< signal .chip_tb.u_apu_ch1.sweep : std_ulogic_vector(6 downto 0) >>));
			dmg.check("nr11", image(<< signal .chip_tb.u_apu_ch1.duty : std_ulogic_vector(1 downto 0) >>));
			dmg.check("nr12", image(<< signal .chip_tb.u_apu_ch1.envelope : std_ulogic_vector(7 downto 0) >>));
			dmg.check("ch1_acc", image(<< signal .chip_tb.u_apu_ch1.acc : std_ulogic_vector(10 downto 0) >>));
			dmg.check("ch1_length_on", image(<< signal .chip_tb.u_apu_ch1.length_on : std_ulogic >>));
			dmg.check("ch1_trigger", image(<< signal .chip_tb.u_apu_ch1.trigger : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch1.start : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch1.triggered : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch1.restart : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch1.restarted : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch1.running : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch1.frozen : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch1.active_i : std_ulogic >>));
			dmg.check("ch1_counter", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch1.counter : unsigned(10 downto 0) >>)));
			dmg.check("ch1_duty", image(<< signal .chip_tb.u_apu_ch1.reload : std_ulogic >>) & image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch1.step : unsigned(2 downto 0) >>))
				& image(<< signal .chip_tb.u_apu_ch1.wave : std_ulogic >>));
			dmg.check("ch1_length", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch1.length : unsigned(5 downto 0) >>)));
			dmg.check("ch1_expired", image(<< signal .chip_tb.u_apu_ch1.expired : std_ulogic >>));
			dmg.check("ch1_envelope", image(<< signal .chip_tb.u_apu_ch1.hz64 : std_ulogic >>) & image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch1.period : unsigned(2 downto 0) >>))
				& image(<< signal .chip_tb.u_apu_ch1.tick : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch1.at_limit : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch1.env_stopped : std_ulogic >>));
			dmg.check("ch1_volume", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch1.volume : unsigned(3 downto 0) >>)));
			dmg.check("ch1_sweep", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch1.sweep_count : unsigned(2 downto 0) >>)) & image(<< signal .chip_tb.u_apu_ch1.sweep_step : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch1.shifting : std_ulogic >>) & image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch1.shifts : unsigned(2 downto 0) >>))
				& image(<< signal .chip_tb.u_apu_ch1.shift_done : std_ulogic >>));
			dmg.check("ch1_shifted", image(<< signal .chip_tb.u_apu_ch1.shifted : std_ulogic_vector(10 downto 0) >>));
			dmg.check("ch1_augend", image(<< signal .chip_tb.u_apu_ch1.augend : std_ulogic_vector(10 downto 0) >>));
			dmg.check("ch1_addend", image(<< signal .chip_tb.u_apu_ch1.addend : std_ulogic_vector(10 downto 0) >>));
			dmg.check("ch1_out", image(ch1_out));
			dmg.check("nr30", image(<< signal .chip_tb.u_apu_ch3.dac_on_i : std_ulogic >>));
			dmg.check("nr32", image(<< signal .chip_tb.u_apu_ch3.level : std_ulogic_vector(1 downto 0) >>));
			dmg.check("ch3_frequency", image(<< signal .chip_tb.u_apu_ch3.frequency : std_ulogic_vector(10 downto 0) >>));
			dmg.check("ch3_length_on", image(<< signal .chip_tb.u_apu_ch3.length_on : std_ulogic >>));
			dmg.check("ch3_trigger", image(<< signal .chip_tb.u_apu_ch3.trigger : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch3.start : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch3.triggered : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch3.restart : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch3.restarted : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch3.running : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch3.frozen : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch3.playing : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch3.active_i : std_ulogic >>));
			dmg.check("ch3_counter", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch3.counter : unsigned(10 downto 0) >>)));
			dmg.check("ch3_position", image(<< signal .chip_tb.u_apu_ch3.reload : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch3.wrapped : std_ulogic >>)
				& image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch3.position : unsigned(4 downto 0) >>)));
			dmg.check("ch3_length", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch3.length : unsigned(7 downto 0) >>)));
			dmg.check("ch3_expired", image(<< signal .chip_tb.u_apu_ch3.expired : std_ulogic >>));
			dmg.check("ch3_fetch", image(<< signal .chip_tb.u_apu_ch3.busa : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch3.bano : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch3.fetch : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch3.azet : std_ulogic >>));
			dmg.check("ch3_sample", image(<< signal .chip_tb.u_apu_ch3.sample_byte : std_ulogic_vector(7 downto 0) >>));
			dmg.check("ch3_out", image(ch3_out));
			dmg.check("nr42", image(<< signal .chip_tb.u_apu_ch4.envelope : std_ulogic_vector(7 downto 0) >>));
			dmg.check("nr43", image(<< signal .chip_tb.u_apu_ch4.noise : std_ulogic_vector(7 downto 0) >>));
			dmg.check("ch4_length_on", image(<< signal .chip_tb.u_apu_ch4.length_on : std_ulogic >>));
			dmg.check("ch4_trigger", image(<< signal .chip_tb.u_apu_ch4.trigger : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch4.start : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch4.triggered : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch4.restart : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch4.restarted : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch4.running : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch4.frozen : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch4.active_i : std_ulogic >>));
			dmg.check("ch4_divider", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch4.divider : unsigned(2 downto 0) >>)) & image(<< signal .chip_tb.u_apu_ch4.divided : std_ulogic >>));
			dmg.check("ch4_prescaler", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch4.prescaler : unsigned(13 downto 0) >>)));
			dmg.check("ch4_lfsr", image(<< signal .chip_tb.u_apu_ch4.lfsr : std_ulogic_vector(15 downto 1) >>)
				& image(<< signal .chip_tb.u_apu_ch4.joto : std_ulogic >>));
			dmg.check("ch4_length", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch4.length : unsigned(5 downto 0) >>)));
			dmg.check("ch4_expired", image(<< signal .chip_tb.u_apu_ch4.expired : std_ulogic >>));
			dmg.check("ch4_envelope", image(<< signal .chip_tb.u_apu_ch4.hz64 : std_ulogic >>) & image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch4.period : unsigned(2 downto 0) >>))
				& image(<< signal .chip_tb.u_apu_ch4.tick : std_ulogic >>) & image(<< signal .chip_tb.u_apu_ch4.at_limit : std_ulogic >>)
				& image(<< signal .chip_tb.u_apu_ch4.env_stopped : std_ulogic >>));
			dmg.check("ch4_volume", image(std_ulogic_vector(<< signal .chip_tb.u_apu_ch4.volume : unsigned(3 downto 0) >>)));
			dmg.check("ch4_out", image(ch4_out));

			-- The data bus
			dmg.check_bus("data", image(std_ulogic_vector(data)));
			wait for 1 ns;
		end loop;
		dmg.finish;
	end process;
end architecture;
