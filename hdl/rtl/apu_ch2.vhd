-- SPDX-License-Identifier: MIT
--
-- The APU's channel 2: a square wave with a length and a volume envelope.
-- NR21 (FF16) sets the duty and the length, NR22 (FF17) the envelope,
-- NR23 and NR24 (FF18, FF19) the frequency, the length's enable and the
-- trigger.
--
-- The frequency counter counts up at 1 MiHz from the frequency, 11 bits;
-- when it overflows it reloads, and the duty steps on. The length counts
-- up at 256 Hz to 64; the envelope steps the volume up or down every 1 to 7
-- ticks of 64 Hz.
--
-- @hw die: dmg-schematics, category apu-ch2 (sheets "Channel 2" and "NR21
-- to NR24"); checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity apu_ch2 is
	port (
		apu_reset : in std_ulogic;
		apu_phi   : in std_ulogic;  -- rises at the start of phase A
		clk_t4    : in std_ulogic;
		ch2_1mhz  : in std_ulogic;
		hz256     : in std_ulogic;  -- the frame sequencer's clocks
		hz128     : in std_ulogic;
		hz512     : in std_ulogic;
		test      : in std_ulogic;  -- NR52 bit 4, test mode

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		active : out std_ulogic;  -- NR52 bit 1
		dac_on : out std_ulogic;  -- NR22's top five bits are not all 0
		output : out std_ulogic_vector(3 downto 0)
	);
end entity;

architecture rtl of apu_ch2 is
	signal write_nr21, write_nr22, write_nr23, write_nr24 : std_ulogic;
	signal write_nr24_late                                : std_ulogic;

	signal duty         : std_ulogic_vector(1 downto 0) := "00";  -- NR21 bits 6, 7
	signal envelope     : std_ulogic_vector(7 downto 0) := (others => '0');  -- NR22
	signal frequency    : std_ulogic_vector(10 downto 0) := (others => '0');  -- NR23, NR24
	signal length_on    : std_ulogic := '0';  -- NR24 bit 6
	signal trigger      : std_ulogic := '0';  -- NR24 bit 7, until the start
	signal dac_on_i     : std_ulogic;

	-- The trigger
	signal start        : std_ulogic := '0';  -- DOPE
	signal triggered    : std_ulogic := '0';  -- ~DALA
	signal restart      : std_ulogic := '0';  -- ELOX
	signal restarted    : std_ulogic := '0';  -- DORY
	signal running      : std_ulogic := '0';  -- CAZA
	signal frozen       : std_ulogic := '1';  -- BUTA: the frequency counter does not count
	signal active_i     : std_ulogic := '0';  -- DANE
	signal stop         : std_ulogic;

	-- The frequency and the duty
	signal counter      : unsigned(10 downto 0) := (others => '0');
	signal count_clock  : std_ulogic;
	signal reload       : std_ulogic := '0';  -- GYKO: the counter overflowed
	signal load_counter : std_ulogic;
	signal step         : unsigned(2 downto 0) := (others => '0');
	signal pwm          : std_ulogic;
	signal wave         : std_ulogic := '0';  -- DOME

	-- The length
	signal length       : unsigned(5 downto 0) := (others => '0');
	signal length_clock : std_ulogic;
	signal expired      : std_ulogic := '0';  -- CYRE
	signal length_reset : std_ulogic;

	-- The envelope
	signal hz64         : std_ulogic := '0';  -- JYNA
	signal period       : unsigned(2 downto 0) := (others => '0');
	signal load_period  : std_ulogic;
	signal tick         : std_ulogic := '0';  -- JOPA
	signal tick_reset   : std_ulogic;
	signal volume       : unsigned(3 downto 0) := (others => '0');
	signal volume_clock : std_ulogic;
	signal volume_clocks : std_ulogic_vector(3 downto 0);
	signal at_limit     : std_ulogic := '0';  -- HEPO
	signal env_stopped  : std_ulogic := '0';  -- JEME
	signal no_period    : std_ulogic;
begin
	-- @hw die: AGYN BACU, ENUF GERE, DOSA EXUC, DETA JENU, EVYF (in G and H)
	write_nr21      <= '1' when cpu_wr = '1' and addr = x"FF16" else '0';
	write_nr22      <= '1' when cpu_wr = '1' and addr = x"FF17" else '0';
	write_nr23      <= '1' when cpu_wr = '1' and addr = x"FF18" else '0';
	write_nr24      <= '1' when cpu_wr = '1' and addr = x"FF19" else '0';
	write_nr24_late <= write_nr24 and clk_t4;

	-- The registers are latches, open while the CPU writes them; NR24's bit 6
	-- only in the last two phases of the write.
	-- @hw die: BERA BAMY, FORE GATA GUFE GURA GAGE HYFU HORE HAVA, FOFE..GUPU,
	-- JEFU JANY JUPY, EMER (drlatch_ee)
	registers : process (all)
	begin
		if apu_reset = '1' then
			duty      <= "00";
			envelope  <= (others => '0');
			frequency <= (others => '0');
			length_on <= '0';
		else
			if write_nr21 = '1' then
				duty <= to_x01(data(7 downto 6));
			end if;
			if write_nr22 = '1' then
				envelope <= to_x01(data);
			end if;
			if write_nr23 = '1' then
				frequency(7 downto 0) <= to_x01(data);
			end if;
			if write_nr24 = '1' then
				frequency(10 downto 8) <= to_x01(data(2 downto 0));
			end if;
			if write_nr24_late = '1' then
				length_on <= to_x01(data(6));
			end if;
		end if;
	end process;

	-- @hw die: FUTE
	dac_on_i <= '1' when envelope(7 downto 3) /= "00000" else '0';
	dac_on   <= dac_on_i;

	----------------------------------------------------------------------
	-- The trigger: NR24 bit 7 is held until the next fall of apu_phi, and
	-- starts the channel; a restart pulse of about a microsecond follows at
	-- the 1 MiHz clock, and the frequency counter counts from the clock
	-- after.
	-- @hw die: ETAP (drlatch_ee), DOPE (dffr); DALA (nor_latch), ELOX DORY
	-- CAZA (dffr), BUTA (nand_latch); DANE (nor_latch), ESYK
	----------------------------------------------------------------------
	trigger_latch : process (all)
	begin
		if apu_reset = '1' or start = '1' then
			trigger <= '0';
		elsif write_nr24 = '1' then
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

	restart_ff : process (ch2_1mhz, apu_reset, restarted)
	begin
		if apu_reset = '1' then
			restarted <= '0';
			running   <= '0';
		elsif rising_edge(ch2_1mhz) then
			restarted <= restart after tco;
			running   <= restarted after tco;
		end if;
		if apu_reset = '1' or restarted = '1' then
			restart <= '0';
		elsif rising_edge(ch2_1mhz) then
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

	active_latch : process (all)
	begin
		if stop = '1' then
			active_i <= '0';
		elsif restart = '1' then
			active_i <= '1';
		end if;
	end process;
	active <= active_i;

	----------------------------------------------------------------------
	-- The frequency counter counts the falls of the 1 MiHz clock, and loads
	-- the frequency while it restarts and while it reloads.
	-- @hw die: DONE DYNU EZOF CYVO FUXO GANO GOCA GANE HEVY HEPU HERO (tffnl),
	-- CAMA DOCA, DUJU COGU EROG GYPA; GYKO (dffr), GALU FUJY GYRE
	----------------------------------------------------------------------
	count_clock  <= ch2_1mhz or frozen;
	load_counter <= reload or restart;

	frequency_counter : process (load_counter, frequency, count_clock)
	begin
		if load_counter = '1' then
			counter <= unsigned(frequency);
		elsif falling_edge(count_clock) then
			counter <= counter + 1 after tco;
		end if;
	end process;

	reload_ff : process (counter(10), apu_reset, restart, ch2_1mhz, reload)
	begin
		if apu_reset = '1' or restart = '1' or (reload = '1' and ch2_1mhz = '1') then
			reload <= '0';
		elsif falling_edge(counter(10)) then
			reload <= '1' after tco;
		end if;
	end process;

	-- The duty steps at the end of each reload, and the wave takes the duty's
	-- value at its start.
	-- @hw die: CANO (dffr), CAGY DYVE (dffr_cc), CULE; EXES DOMO DYTA DOJU
	-- DOVE EGOG DYMU DARE; DOME (dffr)
	duty_counter : process (reload, apu_reset)
	begin
		if apu_reset = '1' then
			step <= (others => '0');
		elsif falling_edge(reload) then
			step <= step + 1 after tco;
		end if;
	end process;

	with duty select pwm <=
		step(2) and step(1) and not step(0) when "00",  -- 12.5%
		step(2) and step(1) when "01",                  -- 25%
		step(2) when "10",                              -- 50%
		not (step(2) and step(1)) when others;          -- 75%

	wave_ff : process (reload, apu_reset)
	begin
		if apu_reset = '1' then
			wave <= '0';
		elsif rising_edge(reload) then
			wave <= pwm after tco;
		end if;
	end process;

	-- @hw die: CYSE BONU, AMOV ASOG ANYV ANAN
	output <= std_ulogic_vector(volume) when (active_i and wave) = '1' or test = '1' else "0000";

	----------------------------------------------------------------------
	-- The length counts at the falls of 256 Hz while it is enabled, and
	-- loads NR21's bits 0 to 5 while the CPU writes NR21; at 64 it expires.
	-- @hw die: ERYC CERA CONU CAME BUVA AKYD (tffnl), DEME DYRO BUKO, BYMO;
	-- CYRE (dffr), BENY
	----------------------------------------------------------------------
	length_clock <= not (length_on and not expired and not hz256);

	length_counter : process (write_nr21, data, length_clock)
	begin
		if write_nr21 = '1' then
			length <= unsigned(to_x01(data(5 downto 0)));
		elsif falling_edge(length_clock) then
			length <= length + 1 after tco;
		end if;
	end process;

	length_reset <= write_nr21 or apu_reset or restart;

	expired_ff : process (length(5), length_reset)
	begin
		if length_reset = '1' then
			expired <= '0';
		elsif falling_edge(length(5)) then
			expired <= not expired after tco;
		end if;
	end process;

	----------------------------------------------------------------------
	-- The envelope.
	----------------------------------------------------------------------
	-- 64 Hz: 128 Hz halved.
	-- @hw die: JYNA (dffr), HOTA
	hz64_ff : process (hz128, apu_reset)
	begin
		if apu_reset = '1' then
			hz64 <= '0';
		elsif falling_edge(hz128) then
			hz64 <= not hz64 after tco;
		end if;
	end process;

	-- The period counts the falls of 64 Hz from NR22's period, inverted, up
	-- to 7; it loads again at the restart and at each tick.
	-- @hw die: JORE JONA JEVY (tffnl), KENE KYLO, HYLY JAKE
	load_period <= restart or tick;

	period_counter : process (load_period, envelope, hz64)
	begin
		if load_period = '1' then
			period <= unsigned(not to_x01(envelope(2 downto 0)));
		elsif falling_edge(hz64) then
			period <= period + 1 after tco;
		end if;
	end process;

	-- A tick is the period at 7 at a rise of 512 Hz, until 512 Hz falls.
	-- @hw die: KYVO, JOPA (dffr), HAFE HOLY JUPU
	no_period  <= '1' when envelope(2 downto 0) = "000" else '0';
	tick_reset <= (not hz512 and tick) or no_period or restart or apu_reset;

	tick_ff : process (hz512, tick_reset)
	begin
		if tick_reset = '1' then
			tick <= '0';
		elsif rising_edge(hz512) then
			tick <= '1' after tco when period = 7 else '0' after tco;
		end if;
	end process;

	-- The volume loads NR22's top four bits at the restart, and steps up or
	-- down (NR22 bit 3) at the end of each tick, until 15 or 0.
	-- @hw die: FENO FETE FOMY FENA (tffnl), GAFA HOFO, FARU ETUP FOPY
	volume_clock <= tick or no_period or env_stopped;

	-- The volume counts as a ripple counter of toggle flip-flops: each stage
	-- toggles at the fall of its clock, which is the stage before it when
	-- counting up, its inverse when counting down. A write that changes the
	-- direction alone thus toggles stages, as on the chip.
	volume_clocks <= std_ulogic_vector(volume(2 downto 0)) & volume_clock when envelope(3) = '1' else
	                 std_ulogic_vector(not volume(2 downto 0)) & volume_clock;

	volume_stages : for i in 0 to 3 generate
		volume_stage : process (restart, envelope(4 + i), volume_clocks(i))
		begin
			if restart = '1' then
				volume(i) <= to_x01(envelope(4 + i));
			elsif falling_edge(volume_clocks(i)) then
				volume(i) <= not volume(i) after tco;
			end if;
		end process;
	end generate;

	-- @hw die: EMYR ERAT FYRE GUFY, HEPO (dffr), HYPA; JEME (nor_latch), HYLE
	limit_ff : process (tick, restart, apu_reset)
	begin
		if restart = '1' or apu_reset = '1' then
			at_limit <= '0';
		elsif rising_edge(tick) then
			at_limit <= '1' after tco when (envelope(3) = '0' and volume = 0)
			                             or (envelope(3) = '1' and volume = 15) else '0' after tco;
		end if;
	end process;

	stop_latch : process (all)
	begin
		if apu_reset = '1' or restart = '1' then
			env_stopped <= '0';
		elsif at_limit = '1' then
			env_stopped <= '1';
		end if;
	end process;

	----------------------------------------------------------------------
	-- Reads: NR21's duty, NR22, NR24's bit 6; the others read as 1. In test
	-- mode, NR23 and NR24 read the frequency counter.
	-- @hw die: CORO CECY CEKA, GURU GEXA HUVU..HYRY, HUMA GOJY; FAPE GUZA
	-- FUTY HYPO, FAVA..GAZO HUNA JARO JEKE
	----------------------------------------------------------------------
	data <= duty & "ZZZZZZ" when cpu_rd = '1' and addr = x"FF16" else
	        envelope when cpu_rd = '1' and addr = x"FF17" else
	        std_ulogic_vector(counter(7 downto 0)) when cpu_rd = '1' and addr = x"FF18" and test = '1' else
	        'Z' & length_on & "ZZZ" & std_ulogic_vector(counter(10 downto 8))
	            when cpu_rd = '1' and addr = x"FF19" and test = '1' else
	        'Z' & length_on & "ZZZZZZ" when cpu_rd = '1' and addr = x"FF19" else
	        (others => 'Z');
end architecture;
