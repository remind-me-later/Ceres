-- SPDX-License-Identifier: MIT
--
-- The APU's channel 3: the wave channel. NR30 (FF1A) turns its DAC on, NR31
-- (FF1B) sets the length, NR32 (FF1C) the output level, NR33 and NR34
-- (FF1D, FF1E) the frequency, the length's enable and the trigger; wave
-- RAM (FF30 to FF3F) holds 32 samples of 4 bits.
--
-- The frequency counter counts up at 2 MiHz; each overflow steps the
-- position in wave RAM, 32 nibbles, high nibble first. While the channel
-- plays, the RAM is addressed by the position, and is read just after each
-- step: a read of wave RAM by the CPU then gets the byte the channel reads.
--
-- @hw die: dmg-schematics, category apu-ch3 (sheets "Channel 3", "NR30 to
-- NR34" and "Wave RAM"), and the cell wave_ram (dmg-sim's simplified
-- SRAM); checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity apu_ch3 is
	port (
		clk       : in std_ulogic;
		apu_reset : in std_ulogic;
		apu_phi   : in std_ulogic;  -- rises at the start of phase A
		clk_t4    : in std_ulogic;
		pch_phase : in std_ulogic;
		ajer      : in std_ulogic;  -- 2 MiHz
		ch3_2mhz  : in std_ulogic;
		hz256     : in std_ulogic;
		test      : in std_ulogic;  -- NR52 bit 4, test mode

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		active : out std_ulogic;  -- NR52 bit 2
		dac_on : out std_ulogic;  -- NR30 bit 7
		output : out std_ulogic_vector(3 downto 0)
	);
end entity;

architecture rtl of apu_ch3 is
	type ram_t is array (0 to 15) of std_ulogic_vector(7 downto 0);

	signal write_nr30, write_nr31, write_nr32, write_nr33, write_nr34 : std_ulogic;
	signal write_nr34_late, wave_area                                 : std_ulogic;

	signal dac_on_i     : std_ulogic := '0';  -- NR30 bit 7
	signal level        : std_ulogic_vector(1 downto 0) := "00";  -- NR32 bits 5, 6
	signal frequency    : std_ulogic_vector(10 downto 0) := (others => '0');  -- NR33, NR34
	signal length_on    : std_ulogic := '0';  -- NR34 bit 6
	signal trigger      : std_ulogic := '0';  -- NR34 bit 7, until the start

	-- The trigger
	signal start        : std_ulogic := '0';  -- FOBA
	signal triggered    : std_ulogic := '0';  -- ~GOFY
	signal restart      : std_ulogic := '0';  -- GARA
	signal restarted    : std_ulogic := '0';  -- GYTA
	signal running      : std_ulogic := '0';  -- GYRA
	signal frozen       : std_ulogic := '1';  -- GUGU
	signal playing      : std_ulogic := '0';  -- FOZU
	signal active_i     : std_ulogic := '0';  -- DAVO: playing, at 2 MiHz
	signal stop         : std_ulogic;

	-- The frequency and the position
	signal counter      : unsigned(10 downto 0) := (others => '0');
	signal count_clock  : std_ulogic;
	signal reload       : std_ulogic := '0';  -- HUNO
	signal load_counter : std_ulogic;
	signal position     : unsigned(4 downto 0) := (others => '0');
	signal wrapped      : std_ulogic := '0';  -- FETY
	signal position_reset : std_ulogic;

	-- The length
	signal length       : unsigned(7 downto 0) := (others => '0');
	signal length_clock : std_ulogic;
	signal expired      : std_ulogic := '0';  -- FEXU
	signal length_reset : std_ulogic;

	-- Wave RAM
	signal ram          : ram_t := (others => (others => '0'));
	signal ram_addr     : std_ulogic_vector(3 downto 0);
	signal busa, bano   : std_ulogic := '0';
	signal fetch        : std_ulogic := '0';  -- AZUS: wave_data_latch
	signal azet         : std_ulogic := '0';
	signal precharge    : std_ulogic;
	signal ram_read     : std_ulogic;
	signal ram_write    : std_ulogic;
	signal word_line    : std_ulogic;
	signal bit_lines    : std_ulogic_vector(7 downto 0) := (others => '1');
	signal sample_byte  : std_ulogic_vector(7 downto 0) := (others => '0');
	signal sample       : std_ulogic_vector(3 downto 0);
begin
	-- @hw die: GEJO, DERY, HAGA, JAFA KOTA, HUDA EPYX, FOVO (in G and H); BYZA
	write_nr30      <= '1' when cpu_wr = '1' and addr = x"FF1A" else '0';
	write_nr31      <= '1' when cpu_wr = '1' and addr = x"FF1B" else '0';
	write_nr32      <= '1' when cpu_wr = '1' and addr = x"FF1C" else '0';
	write_nr33      <= '1' when cpu_wr = '1' and addr = x"FF1D" else '0';
	write_nr34      <= '1' when cpu_wr = '1' and addr = x"FF1E" else '0';
	write_nr34_late <= write_nr34 and clk_t4;
	wave_area       <= '1' when addr(15 downto 4) = x"FF3" else '0';

	-- @hw die: GUXE, HODY HUKY, KOGA..KOGU, JEMO JETY JACY, HOTO (drlatch_ee)
	registers : process (all)
	begin
		if apu_reset = '1' then
			dac_on_i  <= '0';
			level     <= "00";
			frequency <= (others => '0');
			length_on <= '0';
		else
			if write_nr30 = '1' then
				dac_on_i <= to_x01(data(7));
			end if;
			if write_nr32 = '1' then
				level <= to_x01(data(6 downto 5));
			end if;
			if write_nr33 = '1' then
				frequency(7 downto 0) <= to_x01(data);
			end if;
			if write_nr34 = '1' then
				frequency(10 downto 8) <= to_x01(data(2 downto 0));
			end if;
			if write_nr34_late = '1' then
				length_on <= to_x01(data(6));
			end if;
		end if;
	end process;
	dac_on <= dac_on_i;

	----------------------------------------------------------------------
	-- The trigger, as channel 2's, but at the falls of 2 MiHz; the channel
	-- shows as active at the next rise of ajer.
	-- @hw die: GAVU (drlatch_ee), FOBA (dffr); GOFY (nor_latch), GARA GYTA
	-- GYRA (dffr), GUGU (nand_latch); FOZU (nor_latch), FYGO; DAVO (dffr)
	----------------------------------------------------------------------
	trigger_latch : process (all)
	begin
		if apu_reset = '1' or start = '1' then
			trigger <= '0';
		elsif write_nr34 = '1' then
			trigger <= to_x01(data(7));
		end if;
	end process;

	start_ff : process (apu_phi, apu_reset)
	begin
		if apu_reset = '1' then
			start <= '0';
		elsif rising_edge(apu_phi) then
			start <= trigger after tco;
		end if;
	end process;

	triggered_latch : process (all)
	begin
		if apu_reset = '1' or restarted = '1' then
			triggered <= '0';
		elsif start = '1' then
			triggered <= '1';
		end if;
	end process;

	restart_ff : process (ch3_2mhz, apu_reset, restarted)
	begin
		if apu_reset = '1' then
			restarted <= '0';
			running   <= '0';
		else
			if falling_edge(ch3_2mhz) then
				restarted <= restart after tco;
			end if;
			if rising_edge(ch3_2mhz) then
				running <= restarted after tco;
			end if;
		end if;
		if apu_reset = '1' or restarted = '1' then
			restart <= '0';
		elsif falling_edge(ch3_2mhz) then
			restart <= triggered after tco;
		end if;
	end process;

	frozen_latch : process (all)
	begin
		if dac_on_i = '0' or apu_reset = '1' then
			frozen <= '1';
		elsif running = '1' then
			frozen <= '0';
		end if;
	end process;

	stop <= apu_reset or (expired and length_on) or not dac_on_i;

	playing_latch : process (all)
	begin
		if stop = '1' then
			playing <= '0';
		elsif restart = '1' then
			playing <= '1';
		end if;
	end process;

	active_ff : process (ajer, apu_reset)
	begin
		if apu_reset = '1' then
			active_i <= '0';
		elsif rising_edge(ajer) then
			active_i <= playing after tco;
		end if;
	end process;
	active <= active_i;

	----------------------------------------------------------------------
	-- The frequency counter counts the falls of 2 MiHz; each overflow steps
	-- the position, 0 to 31.
	-- @hw die: KUTU KUPE KUNU KEMU KYGU KEPA KAFO KENO KEJU KEZA JAPU
	-- (tffnl), HEFO JUTY, HERA KYKO KASO JERA; HUNO (dffr); EFAR ERUS EFUZ
	-- EXEL EFAL FETY (dffr), ETAN GYRY
	----------------------------------------------------------------------
	count_clock  <= ch3_2mhz or frozen;
	load_counter <= reload or restart;

	frequency_counter : process (load_counter, frequency, count_clock)
	begin
		if load_counter = '1' then
			counter <= unsigned(frequency);
		elsif falling_edge(count_clock) then
			counter <= counter + 1 after tco;
		end if;
	end process;

	reload_ff : process (counter(10), apu_reset, restart, ch3_2mhz, reload)
	begin
		if apu_reset = '1' or restart = '1' or (reload = '1' and ch3_2mhz = '1') then
			reload <= '0';
		elsif falling_edge(counter(10)) then
			reload <= '1' after tco;
		end if;
	end process;

	position_reset <= restart or wrapped;

	position_counter : process (reload, position_reset)
	begin
		if position_reset = '1' then
			position <= (others => '0');
		elsif falling_edge(reload) then
			position <= position + 1 after tco;
		end if;
	end process;

	wrapped_ff : process (position(4), apu_reset, restart, reload, wrapped)
	begin
		if apu_reset = '1' or restart = '1' or (wrapped = '1' and reload = '1') then
			wrapped <= '0';
		elsif falling_edge(position(4)) then
			wrapped <= not wrapped after tco;
		end if;
	end process;

	----------------------------------------------------------------------
	-- The length counts the falls of 256 Hz from NR31, 8 bits, up to 256.
	-- @hw die: GEVO FORY GATU GAPO GEMO FORO FAVE FYRU (tffnl), GEPY GENU,
	-- GAJY EMUT; FEXU (dffr), GUDA
	----------------------------------------------------------------------
	length_clock <= not (length_on and not expired and not hz256);

	length_counter : process (write_nr31, data, length_clock)
	begin
		if write_nr31 = '1' then
			length <= unsigned(to_x01(data));
		elsif falling_edge(length_clock) then
			length <= length + 1 after tco;
		end if;
	end process;

	length_reset <= write_nr31 or apu_reset or restart;

	expired_ff : process (length(7), length_reset)
	begin
		if length_reset = '1' then
			expired <= '0';
		elsif falling_edge(length(7)) then
			expired <= not expired after tco;
		end if;
	end process;

	----------------------------------------------------------------------
	-- Wave RAM. While the channel plays, it is addressed by the position and
	-- read at each step: a reload pulse, delayed by half clocks, opens the
	-- latches of the sample (fetch). Otherwise the CPU addresses it.
	-- @hw die: BUSA BANO AZUS AZET (dffr), BUTU; AFUM AGYL AXOL BOLE (mux);
	-- ATUR BORU BUKU BETA BORY, ALER YRAR; BENO ATOK, YMUL YLAC YTYL;
	-- wave_ram (simplified_sram)
	----------------------------------------------------------------------
	fetch_ff : process (clk, apu_reset)
	begin
		if apu_reset = '1' then
			busa  <= '0';
			bano  <= '0';
			fetch <= '0';
			azet  <= '0';
		else
			if falling_edge(clk) then
				busa  <= reload after tco;
				fetch <= bano after tco;
			end if;
			if rising_edge(clk) then
				bano <= busa after tco;
				azet <= fetch after tco;
			end if;
		end if;
	end process;

	ram_addr  <= std_ulogic_vector(position(4 downto 1)) when active_i = '1' else addr(3 downto 0);
	ram_read  <= fetch when active_i = '1' else cpu_rd and wave_area;
	ram_write <= cpu_wr and wave_area;
	precharge <= not (fetch or azet) when active_i = '1' else
	             not (ram_write or ram_read or not pch_phase);
	word_line <= not precharge and (ram_read or (ram_write and not active_i));

	ram_cells : process (all)
		variable i : natural range 0 to 15;
	begin
		i := to_integer(to_01(unsigned(ram_addr)));
		if precharge = '1' then
			bit_lines <= (others => '1');
		elsif ram_write = '1' then
			bit_lines <= to_x01(data);
			if word_line = '1' then
				ram(i) <= to_x01(data);
			end if;
		elsif word_line = '1' then
			bit_lines <= ram(i);
		end if;
	end process;

	sample_latch : process (all)
	begin
		if apu_reset = '1' then
			sample_byte <= (others => '0');
		elsif fetch = '1' then
			sample_byte <= bit_lines;
		end if;
	end process;

	-- The high nibble first.
	-- @hw die: DATE DAZY CUZO COPO (muxi), EFAR (wave_nibble_sel)
	sample <= sample_byte(3 downto 0) when position(0) = '1' else sample_byte(7 downto 4);

	-- The level: mute, 100%, 50%, 25%.
	-- @hw die: GEGE GOKA GEMY, EZAG EVUG DOKY DORE, BARY BYKA BOPA BELY
	with level select output <=
		"0000" when "00",
		sample and (3 downto 0 => active_i) when "01",
		('0' & sample(3 downto 1)) and (3 downto 0 => active_i) when "10",
		("00" & sample(3 downto 2)) and (3 downto 0 => active_i) when others;

	----------------------------------------------------------------------
	-- Reads: NR30's bit 7, NR32's level, NR34's bit 6, and wave RAM: its bit
	-- lines, which while the channel plays hold the byte it reads during the
	-- fetch, and are precharged (FF) otherwise.
	-- @hw die: FASY GEKO, HENU HAMU HUCO, GAWA HACA; DUGU..BEZU (not_if1)
	----------------------------------------------------------------------
	data <= dac_on_i & "ZZZZZZZ" when cpu_rd = '1' and addr = x"FF1A" else
	        'Z' & level & "ZZZZZ" when cpu_rd = '1' and addr = x"FF1C" else
	        'Z' & length_on & "ZZZZZZ" when cpu_rd = '1' and addr = x"FF1E" else
	        bit_lines when cpu_rd = '1' and wave_area = '1' else
	        (others => 'Z');
end architecture;
