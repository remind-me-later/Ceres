-- SPDX-License-Identifier: MIT
--
-- The APU's channel 4: noise, with a length and a volume envelope. NR41
-- (FF20) sets the length, NR42 (FF21) the envelope, NR43 (FF22) the noise's
-- clock and width, NR44 (FF23) the length's enable and the trigger.
--
-- A divider of 512 KiHz (NR43 bits 0 to 2) clocks a 14-bit prescaler, whose
-- bit NR43 bits 4 to 7 select clocks the noise: a shift register of 15
-- stages, with the feedback (bit 13 xnor bit 14) into its first stage, and,
-- in the 7-bit width (NR43 bit 3), into its eighth too.
--
-- @hw die: dmg-schematics, category apu-ch4 (sheets "Channel 4" and "NR41
-- to NR44"); checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity apu_ch4 is
	port (
		apu_reset : in std_ulogic;
		apu_phi   : in std_ulogic;  -- rises at the start of phase A
		clk_t4    : in std_ulogic;
		ch4_1mhz  : in std_ulogic;
		khz512    : in std_ulogic;
		hz256     : in std_ulogic;
		hz128     : in std_ulogic;
		hz512     : in std_ulogic;
		test      : in std_ulogic;  -- NR52 bit 4, test mode

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		active : out std_ulogic;  -- NR52 bit 3
		dac_on : out std_ulogic;
		output : out std_ulogic_vector(3 downto 0)
	);
end entity;

architecture rtl of apu_ch4 is
	signal write_nr41, write_nr42, write_nr43, write_nr44 : std_ulogic;
	signal write_nr44_late                                : std_ulogic;

	signal envelope     : std_ulogic_vector(7 downto 0) := (others => '0');  -- NR42
	signal noise        : std_ulogic_vector(7 downto 0) := (others => '0');  -- NR43
	signal length_on    : std_ulogic := '0';  -- NR44 bit 6
	signal trigger      : std_ulogic := '0';  -- NR44 bit 7, until the start
	signal dac_on_i     : std_ulogic;

	-- The trigger
	signal start        : std_ulogic := '0';  -- GYSU
	signal triggered    : std_ulogic := '0';  -- ~HAZO
	signal restart      : std_ulogic := '0';  -- GONE
	signal restarted    : std_ulogic := '0';  -- GORA
	signal running      : std_ulogic := '0';  -- GATY
	signal frozen       : std_ulogic := '1';  -- JERY
	signal active_i     : std_ulogic := '0';  -- GENA
	signal stop         : std_ulogic;

	-- The noise's clock
	signal divider      : unsigned(2 downto 0) := (others => '0');
	signal divider_clock : std_ulogic;
	signal load_divider : std_ulogic;
	signal divided      : std_ulogic := '0';  -- GARY
	signal counter_clock : std_ulogic;
	signal prescaler    : unsigned(13 downto 0) := (others => '0');
	signal shift_clock  : std_ulogic;  -- ch4_lfsr_clk1
	signal bypass       : std_ulogic;

	-- The noise
	signal lfsr         : std_ulogic_vector(15 downto 1) := (others => '0');
	signal joto         : std_ulogic := '0';  -- the first stage, on the other edge
	signal feedback     : std_ulogic;

	-- The length
	signal length       : unsigned(5 downto 0) := (others => '0');
	signal length_clock : std_ulogic;
	signal expired      : std_ulogic := '0';  -- FUGO
	signal length_reset : std_ulogic;

	-- The envelope
	signal hz64         : std_ulogic := '0';  -- ABEL
	signal period       : unsigned(2 downto 0) := (others => '0');
	signal load_period  : std_ulogic;
	signal tick         : std_ulogic := '0';  -- FOSY
	signal tick_reset   : std_ulogic;
	signal volume       : unsigned(3 downto 0) := (others => '0');
	signal volume_clock : std_ulogic;
	signal volume_clocks : std_ulogic_vector(3 downto 0);
	signal at_limit     : std_ulogic := '0';  -- FYNO
	signal env_stopped  : std_ulogic := '0';  -- EROX
	signal no_period    : std_ulogic;
	signal wave         : std_ulogic;
begin
	-- @hw die: CAZE, DACO GOKO, GETU HUMO HOSO, FOXE, DULU (in G and H)
	write_nr41      <= '1' when cpu_wr = '1' and addr = x"FF20" else '0';
	write_nr42      <= '1' when cpu_wr = '1' and addr = x"FF21" else '0';
	write_nr43      <= '1' when cpu_wr = '1' and addr = x"FF22" else '0';
	write_nr44      <= '1' when cpu_wr = '1' and addr = x"FF23" else '0';
	write_nr44_late <= write_nr44 and clk_t4;

	-- @hw die: EMOK ETYJ EZYK GEKY GARU GOKY GOZO GEDU, JARE JERO JAKY JAMY FETA
	-- FYTO GOGO GAFO, CUNY (drlatch_ee)
	registers : process (all)
	begin
		if apu_reset = '1' then
			envelope  <= (others => '0');
			noise     <= (others => '0');
			length_on <= '0';
		else
			if write_nr42 = '1' then
				envelope <= to_x01(data);
			end if;
			if write_nr43 = '1' then
				noise <= to_x01(data);
			end if;
			if write_nr44_late = '1' then
				length_on <= to_x01(data(6));
			end if;
		end if;
	end process;

	-- @hw die: GEVY
	dac_on_i <= '1' when envelope(7 downto 3) /= "00000" else '0';
	dac_on   <= dac_on_i;

	----------------------------------------------------------------------
	-- The trigger, as channel 2's, but at 512 KiHz.
	-- @hw die: HOGA (drlatch_ee), GYSU (dffr); HAZO (nor_latch), GONE GORA
	-- GATY (dffr), JERY (nand_latch); GENA (nor_latch), FEGY
	----------------------------------------------------------------------
	trigger_latch : process (all)
	begin
		if apu_reset = '1' or start = '1' then
			trigger <= '0';
		elsif write_nr44 = '1' then
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

	restart_ff : process (khz512, apu_reset, restarted)
	begin
		if apu_reset = '1' then
			restarted <= '0';
			running   <= '0';
		elsif rising_edge(khz512) then
			restarted <= restart after tco;
			running   <= restarted after tco;
		end if;
		if apu_reset = '1' or restarted = '1' then
			restart <= '0';
		elsif rising_edge(khz512) then
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
	-- The noise's clock. The divider counts the falls of 512 KiHz from NR43
	-- bits 0 to 2, inverted, up to 7, and loads again at each fall of the
	-- 1 MiHz clock after that; the prescaler counts those 1 MiHz pulses.
	-- @hw die: JYCO JYRE JYFU (tffnl), KYKU KONY KANU, GOFU HUCE; HYNO, GARY
	-- (dffr), GYBA; CARY; CEXO DEKO EZEF EPOR DURE DALE DOKE DEMO DOSE DETE
	-- ERUT DOTA DERE ESEP (dffr); the decoders of NR43 bits 4 to 7, DARY ELYX
	-- ETYR EYRF, FEME (mux)
	----------------------------------------------------------------------
	divider_clock <= frozen or khz512;
	load_divider  <= restart or divided;

	divider_counter : process (load_divider, noise, divider_clock)
	begin
		if load_divider = '1' then
			divider <= unsigned(not to_x01(noise(2 downto 0)));
		elsif falling_edge(divider_clock) then
			divider <= divider + 1 after tco;
		end if;
	end process;

	divided_ff : process (ch4_1mhz, apu_reset, restart)
	begin
		if apu_reset = '1' or restart = '1' then
			divided <= '0';
		elsif falling_edge(ch4_1mhz) then
			divided <= '1' after tco when divider = 7 else '0' after tco;
		end if;
	end process;

	counter_clock <= ch4_1mhz and divided;

	prescaler_counter : process (counter_clock, apu_reset)
	begin
		if apu_reset = '1' then
			prescaler <= (others => '0');
		elsif rising_edge(counter_clock) then
			prescaler <= prescaler + 1 after tco;
		end if;
	end process;

	-- Bits 4 to 6 of NR43 select bit 0 to 7 of the prescaler; with bit 7,
	-- bit 8 to 13 (and nothing above).
	clock_select : process (all)
		variable shift : natural range 0 to 15;
	begin
		shift := to_integer(to_01(unsigned(noise(7 downto 4))));
		if shift <= 13 then
			shift_clock <= prescaler(shift);
		else
			shift_clock <= '0';
		end if;
	end process;

	-- Test mode: the prescaler's clock goes out instead of the noise.
	-- @hw die: DYRY CEPY COTE
	bypass <= length_on and test;

	----------------------------------------------------------------------
	-- The noise: the stages shift at the rise of the selected clock; the
	-- first one takes the feedback at the fall. A restart clears them all.
	-- @hw die: JOTO KOMU KETU KUTA KUZY KYWY JAJU HAPE JUXE JEPE JAVO HEPA
	-- HORY HENO HYRO HEZU (dffr), HURA (xnor), KAVU, GOGE
	----------------------------------------------------------------------
	feedback <= lfsr(14) xnor lfsr(15);

	first_stage : process (shift_clock, restart, apu_reset)
	begin
		if restart = '1' or apu_reset = '1' then
			joto <= '0';
		elsif falling_edge(shift_clock) then
			joto <= feedback after tco;
		end if;
	end process;

	stages : process (shift_clock, restart, apu_reset)
	begin
		if restart = '1' or apu_reset = '1' then
			lfsr <= (others => '0');
		elsif rising_edge(shift_clock) then
			lfsr(1) <= joto after tco;
			for i in 2 to 15 loop
				if i = 9 and noise(3) = '1' then
					lfsr(i) <= joto after tco;
				else
					lfsr(i) <= lfsr(i - 1) after tco;
				end if;
			end loop;
		end if;
	end process;

	-- @hw die: GAME, EZUL (mux), DATO, AKOF BYZY APYR BOZA
	wave   <= shift_clock when bypass = '1' else active_i and lfsr(15);
	output <= std_ulogic_vector(volume) when (wave or (test and not length_on)) = '1' else "0000";

	----------------------------------------------------------------------
	-- The length, as channel 2's.
	-- @hw die: DANO FAVY DENA CEDO FYLO EDOP (tffnl), DODA CUWA, DOTU EPEK;
	-- FUGO (dffr), GAPY
	----------------------------------------------------------------------
	length_clock <= not (length_on and not expired and not hz256);

	length_counter : process (write_nr41, data, length_clock)
	begin
		if write_nr41 = '1' then
			length <= unsigned(to_x01(data(5 downto 0)));
		elsif falling_edge(length_clock) then
			length <= length + 1 after tco;
		end if;
	end process;

	length_reset <= write_nr41 or apu_reset or restart;

	expired_ff : process (length(5), length_reset)
	begin
		if length_reset = '1' then
			expired <= '0';
		elsif falling_edge(length(5)) then
			expired <= not expired after tco;
		end if;
	end process;

	----------------------------------------------------------------------
	-- The envelope, as channel 2's.
	-- @hw die: ABEL (dffr), ALOP; CUNA COFE DOGO (tffnl), ENEC DAPY; FOSY
	-- (dffr), GOPA HURY FOWA; FEKO FATY FERU FYRO (tffnl), FELO; FYNO (dffr),
	-- EROX (nor_latch)
	----------------------------------------------------------------------
	hz64_ff : process (hz128, apu_reset)
	begin
		if apu_reset = '1' then
			hz64 <= '0';
		elsif falling_edge(hz128) then
			hz64 <= not hz64 after tco;
		end if;
	end process;

	load_period <= restart or tick;

	period_counter : process (load_period, envelope, hz64)
	begin
		if load_period = '1' then
			period <= unsigned(not to_x01(envelope(2 downto 0)));
		elsif falling_edge(hz64) then
			period <= period + 1 after tco;
		end if;
	end process;

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
	-- Reads: NR42, NR43, NR44's bit 6.
	-- @hw die: HASU, HEZE KEKA JORA, BARE CURY; BAGU ATEL (test)
	----------------------------------------------------------------------
	data <= envelope when cpu_rd = '1' and addr = x"FF21" else
	        noise when cpu_rd = '1' and addr = x"FF22" else
	        'Z' & length_on & "ZZZZZ" & (counter_clock and bypass) when cpu_rd = '1' and addr = x"FF23" and test = '1' else
	        'Z' & length_on & "ZZZZZZ" when cpu_rd = '1' and addr = x"FF23" else
	        (others => 'Z');
end architecture;
