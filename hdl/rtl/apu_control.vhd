-- SPDX-License-Identifier: MIT
--
-- The APU's control: NR50 (FF24, the volumes), NR51 (FF25, which channel
-- goes to which side), NR52 (FF26, the APU's power and the channels'
-- status); the APU's clocks and resets; the frame sequencer's clocks of
-- 512, 256 and 128 Hz; and the mixer, analog on the chip.
--
-- @hw die: dmg-schematics, categories apu-control and apu-analog (sheets
-- "APU control" and "Analog"); the mixer is dmg-sim's model of the analog
-- part (audio.sv); checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use ieee.math_real.all;

use work.clocking.all;

entity apu_control is
	port (
		clk        : in std_ulogic;
		reset2     : in std_ulogic;
		data_phase : in std_ulogic;
		clk_t4     : in std_ulogic;
		div        : in unsigned(15 downto 0);
		t1_only    : in std_ulogic;

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		apu_reset : out std_ulogic;  -- NR52 bit 7 clear, or reset2
		apu_phi   : out std_ulogic;  -- not data_phase: rises at the start of phase A
		test      : out std_ulogic;  -- NR52 bit 4, test mode

		-- The channels' clocks
		ajer     : out std_ulogic;  -- 2 MiHz
		ch1_1mhz : out std_ulogic;
		ch2_1mhz : out std_ulogic;
		ch3_2mhz : out std_ulogic;
		ch4_1mhz : out std_ulogic;
		khz512   : out std_ulogic;  -- 512 KiHz

		-- The frame sequencer's clocks
		hz512      : out std_ulogic;
		hz256      : out std_ulogic;
		hz128      : out std_ulogic;
		hz128_ch1  : out std_ulogic;  -- for channel 1's sweep

		-- The channels
		active  : in std_ulogic_vector(4 downto 1);
		dac_on  : in std_ulogic_vector(4 downto 1);
		ch1, ch2, ch3, ch4 : in std_ulogic_vector(3 downto 0);
		vin     : in real;   -- the cartridge's sound, in volts

		so1, so2 : out real  -- the right and left outputs, in volts
	);
end entity;

architecture rtl of apu_control is
	signal write_nr50, write_nr51, write_nr52 : std_ulogic;
	signal power             : std_ulogic := '0';  -- NR52 bit 7: HADA
	signal apu_reset_i       : std_ulogic;
	signal test_i            : std_ulogic := '0';  -- FERO
	signal test_bit5         : std_ulogic := '0';  -- BOWY
	signal baza              : std_ulogic := '0';
	signal nr50, nr51        : std_ulogic_vector(7 downto 0) := (others => '0');

	signal ajer_i, calo      : std_ulogic := '0';
	signal atep, cemo        : std_ulogic := '0';
	signal cery              : std_ulogic := '0';
	signal atyk, avok        : std_ulogic := '0';
	signal jeso              : std_ulogic := '0';
	signal ch4_1mhz_i        : std_ulogic;
	signal bara, caru, bylu  : std_ulogic := '0';
	signal hz128_i           : std_ulogic;

	signal amp_en            : std_ulogic;
begin
	-- @hw die: BOSU BOWE, BUPO BONO, ETUC HAWU BOPY
	write_nr50 <= '1' when cpu_wr = '1' and addr = x"FF24" else '0';
	write_nr51 <= '1' when cpu_wr = '1' and addr = x"FF25" else '0';
	write_nr52 <= '1' when cpu_wr = '1' and addr = x"FF26" else '0';

	-- NR52 bit 7 powers the APU: while it is clear (and in reset2), the APU
	-- is held in reset. It is written when the CPU's write ends.
	-- @hw die: HADA (dffr), HAWU GUFO HAPO; JYRO KUBY KEBA KEPY
	power_ff : process (write_nr52, reset2)
	begin
		if reset2 = '1' then
			power <= '0';
		elsif falling_edge(write_nr52) then
			power <= to_x01(data(7)) after tco;
		end if;
	end process;

	apu_reset_i <= not power or reset2;
	apu_reset   <= apu_reset_i;

	-- Test mode: NR52 bit 4, writable with T1 alone, and bit 5.
	-- @hw die: FERO (drlatch_ee), EFOP; BOWY (dffr), BAZA (dffr)
	test_latch : process (all)
	begin
		if apu_reset_i = '1' then
			test_i <= '0';
		elsif write_nr52 = '1' then
			test_i <= to_x01(data(4)) and t1_only;
		end if;
	end process;
	test <= test_i;

	test_ff : process (write_nr52, ajer_i, apu_reset_i)
	begin
		if apu_reset_i = '1' then
			test_bit5 <= '0';
			baza      <= '0';
		else
			if falling_edge(write_nr52) then
				test_bit5 <= to_x01(data(5)) after tco;
			end if;
			if falling_edge(ajer_i) then
				baza <= test_bit5 after tco;
			end if;
		end if;
	end process;

	-- NR50 and NR51 are latches, open while the CPU writes them.
	-- @hw die: APEG BYGA AGER APOS BYRE BUMO COZU BEDU, ANEV BOGU BAFO ATUF BUME
	-- BOFA BEFO BEPU (drlatch_ee)
	registers : process (all)
	begin
		if apu_reset_i = '1' then
			nr50 <= (others => '0');
			nr51 <= (others => '0');
		else
			if write_nr50 = '1' then
				nr50 <= to_x01(data);
			end if;
			if write_nr51 = '1' then
				nr51 <= to_x01(data);
			end if;
		end if;
	end process;

	----------------------------------------------------------------------
	-- The clocks. Each channel has its own divider of the chip's clock,
	-- all of them held in reset with the APU.
	-- @hw die: AJER CALO ATEP CEMO CERY ATYK AVOK JESO (dffr); APUV BATA
	-- DYFA AZEG BUFO CYBO ARYF BAVU AMUK
	----------------------------------------------------------------------
	dividers : process (clk, apu_reset_i)
	begin
		if apu_reset_i = '1' then
			ajer_i <= '0';
			atep   <= '0';
			cery   <= '0';
			atyk   <= '0';
		elsif rising_edge(clk) then
			ajer_i <= not ajer_i after tco;
			atep   <= not atep after tco;
			cery   <= not cery after tco;
			atyk   <= not atyk after tco;
		end if;
	end process;
	ajer     <= ajer_i;
	ch3_2mhz <= cery;

	half_dividers : process (ajer_i, atep, atyk, apu_reset_i)
	begin
		if apu_reset_i = '1' then
			calo <= '0';
			cemo <= '0';
			avok <= '0';
		else
			if falling_edge(ajer_i) then
				calo <= not calo after tco;
			end if;
			if falling_edge(atep) then
				cemo <= not cemo after tco;
			end if;
			if falling_edge(atyk) then
				avok <= not avok after tco;
			end if;
		end if;
	end process;
	ch1_1mhz   <= calo;
	ch2_1mhz   <= cemo;
	ch4_1mhz_i <= not avok;
	ch4_1mhz   <= ch4_1mhz_i;

	khz512_ff : process (ch4_1mhz_i, apu_reset_i)
	begin
		if apu_reset_i = '1' then
			jeso <= '0';
		elsif rising_edge(ch4_1mhz_i) then
			jeso <= not jeso after tco;
		end if;
	end process;
	khz512 <= jeso;

	-- The frame sequencer: 512 Hz is bit 10 of the divider (inverted),
	-- sampled at 1 MiHz; 256 and 128 Hz divide it. In test mode, they are
	-- all 512 KiHz.
	-- @hw die: BARA CARU BYLU (dffr), COKE BURE CULO APEF; BEZE BULE GALE CELY
	-- (mux), BUFY BYFE HORU CATE
	sequencer : process (ajer_i, apu_reset_i)
	begin
		if apu_reset_i = '1' then
			bara <= '0';
		elsif falling_edge(ajer_i) then
			bara <= not div(10) after tco;
		end if;
	end process;

	sequencer_div : process (bara, caru, apu_reset_i)
	begin
		if apu_reset_i = '1' then
			caru <= '0';
			bylu <= '0';
		else
			if rising_edge(bara) then
				caru <= not caru after tco;
			end if;
			if falling_edge(caru) then
				bylu <= not bylu after tco;
			end if;
		end if;
	end process;

	hz512     <= jeso when test_i = '1' else not bara;
	hz256     <= jeso when test_i = '1' else caru;
	hz128_i   <= jeso when test_i = '1' else bylu;
	hz128     <= hz128_i;
	hz128_ch1 <= baza when test_i = '1' else hz128_i;

	apu_phi <= not data_phase;

	-- The status: the power and the channels.
	-- @hw die: JURE HOPE, DOLE COTO DURU EFUS FEWA FATE KAMU KOGE; ADAK, GUMU
	data <= power & "ZZZ" & active(4) & active(3) & active(2) & active(1)
	            when cpu_rd = '1' and addr = x"FF26" else
	        nr50 when cpu_rd = '1' and addr = x"FF24" else
	        nr51 when cpu_rd = '1' and addr = x"FF25" else
	        (others => 'Z');

	----------------------------------------------------------------------
	-- The mixer. Each channel's DAC gives 0.7 to 3.7 V; NR51 picks the
	-- channels for each side, NR50 their volume, around 2.2 V.
	-- @hw die: TACE TYGU (amp_en); @hw inferred: the analog part is dmg-sim's
	-- model (audio.sv), not the netlist
	----------------------------------------------------------------------
	amp_en <= dac_on(1) or dac_on(2) or dac_on(3) or dac_on(4);

	mixer : process (all)
		type volts_t is array (1 to 4) of real;
		variable dac          : volts_t;
		variable right, left  : real;
	begin
		dac(1) := real(to_integer(unsigned(to_01(ch1)))) / 5.0 + 0.7;
		dac(2) := real(to_integer(unsigned(to_01(ch2)))) / 5.0 + 0.7;
		dac(3) := real(to_integer(unsigned(to_01(ch3)))) / 5.0 + 0.7;
		dac(4) := real(to_integer(unsigned(to_01(ch4)))) / 5.0 + 0.7;
		right := 0.0;
		left  := 0.0;
		for i in 1 to 4 loop
			if nr51(i - 1) = '1' and dac_on(i) = '1' then
				right := right + (dac(i) - 2.2) * 0.25;
			end if;
			if nr51(i + 3) = '1' and dac_on(i) = '1' then
				left := left + (dac(i) - 2.2) * 0.25;
			end if;
		end loop;
		if nr50(3) = '1' then
			right := right + (vin - 2.2) * 0.25;
		end if;
		if nr50(7) = '1' then
			left := left + (vin - 2.2) * 0.25;
		end if;
		right := right * (-real(to_integer(unsigned(nr50(2 downto 0))) + 1) / 8.0) + 2.2;
		left  := left * (-real(to_integer(unsigned(nr50(6 downto 4))) + 1) / 8.0) + 2.2;
		if amp_en = '1' then
			so1 <= realmax(0.3, realmin(4.3, right));
			so2 <= realmax(0.3, realmin(4.3, left));
		else
			so1 <= 2.2;
			so2 <= 2.2;
		end if;
	end process;
end architecture;
