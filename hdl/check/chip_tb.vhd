-- SPDX-License-Identifier: MIT
--
-- Checks the model against dmg-sim, phase by phase. dmg-sim's samples
-- (ORACLE, see ../oracle/probe_phases.sv) give the inputs: the crystal, the
-- pins, and what is not modelled yet: the CPU core, the APU and the PPU's
-- rendering. At the end of every phase, the model's signals must have
-- dmg-sim's values. The blocks of the model drive each other, so that what
-- passes between them within a phase (such as the STAT write glitch)
-- counts.
--
-- Within a phase, the inputs follow dmg-sim's timing roughly: the CPU
-- core's outputs come 15 ns after the clock's edge, its address's high
-- byte at 22 ns and low byte at 33 ns, and its data at 40 ns. The internal
-- data bus is precharged high in phase A (external_bus), and keeps its
-- value while nobody drives it. The CPU core pulls the bits that are 0 on
-- its side down whenever it does not read (when it writes, from phase B or
-- E): the latches that a write opens see the precharged FF first.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;
use work.oracle.all;

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
	signal read, write, ctl_mreq     : std_ulogic := '0';
	signal mreq, fexx_ffxx           : std_ulogic;
	signal boot_slow                 : std_ulogic := '1';
	signal osc_ena                   : std_ulogic := '1';
	signal clk_ena                   : std_ulogic := '0';
	signal acks                      : std_ulogic_vector(4 downto 0) := (others => '0');
	signal addr                      : std_logic_vector(15 downto 0);
	signal data                      : std_logic_vector(7 downto 0);

	-- Not modelled yet
	signal mode2, mode3, mode0 : std_ulogic := '0';
	signal int_jp_unused       : std_ulogic;

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

	-- The bus keeper
	signal kept : std_ulogic_vector(7 downto 0) := (others => '1');

	shared variable dmg : oracle_t;

	-- The addresses whose reads the model answers (or that nothing answers).
	function modelled(a : std_ulogic_vector(15 downto 0)) return boolean is
		constant n : natural := to_integer(to_01(unsigned(a)));
	begin
		return not ((n >= 16#8000# and n <= 16#9FFF#)   -- VRAM
		         or (n >= 16#FE00# and n <= 16#FEFF#)   -- OAM
		         or (n >= 16#FF10# and n <= 16#FF3F#)   -- the APU
		         or n = 16#FF42# or n = 16#FF43#        -- SCY, SCX
		         or (n >= 16#FF47# and n <= 16#FF4B#)   -- palettes, WY, WX
		         or n = 16#FFFF#);                      -- IE, in the CPU core
	end function;
begin
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

	-- The CPU core's request goes out unless the address is the boot ROM's
	-- or FE00 to FFFF, which the core decodes itself. The decoding is slower
	-- than the request, so that the request is high for a moment at the
	-- start of such accesses: enough for the latches of the address pins.
	-- (Both decodings take about 10 ns in dmg-sim.)
	-- @hw die: the CPU core's ZLYZ (oa211), the chip's TUNA SYRO, TULO TUTU
	fexx_ffxx  <= '1' after 10 ns when addr(15 downto 9) = "1111111" else '0' after 10 ns;
	boot_slow  <= boot_sel after 10 ns;
	mreq       <= transport ctl_mreq and not fexx_ffxx and (not boot_slow or t2_only) after 8 ns;

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
		variable value   : std_ulogic_vector(7 downto 0);
		variable address : std_ulogic_vector(15 downto 0);
		variable p       : phase_t;
	begin
		dmg.open_file(ORACLE);
		while dmg.next_phase loop
			-- The edge that starts the phase. The rest of the phase follows
			-- dmg-sim's timing, roughly: the CPU core's outputs 15 ns after
			-- the edge, its address's high byte at 22 ns and low byte at 33 ns,
			-- and its data at 40 ns.
			ck1 <= dmg.bit_value("ck1");
			if dmg.phase > 0 then
				wait for 1 ns;
			end if;

			reset_pin_n <= dmg.bit_value("reset_n");
			t1          <= dmg.bit_value("t1");
			t2          <= dmg.bit_value("t2");
			mode2       <= dmg.bit_value("mode2");
			mode3       <= dmg.bit_value("mode3");
			mode0       <= dmg.bit_value("mode0");

			-- The cartridge and the work RAM drive the data pins when the chip
			-- does not.
			if dmg.bit_value("pins_out") = '1' then
				d_pins <= (others => 'Z');
			else
				d_pins <= dmg.vector_value("d_pins");
			end if;

			if dmg.phase > 0 then
				wait for 14 ns;
			end if;
			osc_ena  <= dmg.bit_value("osc_ena");
			clk_ena  <= dmg.bit_value("clk_ena");
			ctl_mreq <= dmg.bit_value("ctl_mreq");
			write    <= dmg.bit_value("write");
			read     <= dmg.bit_value("read");
			acks     <= dmg.vector_value("int_acks");

			address := dmg.vector_value("addr");
			if dmg.phase > 0 then
				wait for 7 ns;
			end if;
			addr(15 downto 8) <= address(15 downto 8);
			if dmg.phase > 0 then
				wait for 11 ns;
			end if;
			addr(7 downto 0) <= address(7 downto 0);

			-- The blocks that are not modelled answer the CPU's reads with
			-- dmg-sim's value. Outside reads, the CPU core's own data bus is
			-- bridged onto the chip's in phases B to H: the core pulls the bits
			-- down that are 0 on its side, as when it writes.
			if dmg.phase > 0 then
				wait for 7 ns;
			end if;
			p     := dmg.phase_value;
			value := dmg.vector_value("data");
			if dmg.bit_value("cpu_rd") = '1' then
				if not modelled(dmg.vector_value("addr")) then
					data <= value;
				else
					data <= (others => 'Z');
				end if;
			elsif p /= A then
				for i in data'range loop
					data(i) <= '0' when value(i) = '0' else 'Z';
				end loop;
			else
				data <= (others => 'Z');
			end if;

			if dmg.phase > 0 then
				wait for T_phase - 41 ns;
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

			-- The CPU core's request, which the testbench computes like the core
			dmg.check("mreq", image(mreq));

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

			-- The data bus
			dmg.check_bus("data", image(std_ulogic_vector(data)));
			wait for 1 ns;
		end loop;
		dmg.finish;
	end process;
end architecture;
