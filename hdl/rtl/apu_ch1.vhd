-- SPDX-License-Identifier: MIT
--
-- The APU's channel 1: channel 2's square wave, length and envelope, with
-- a frequency sweep. NR10 (FF10) sets the sweep, NR11 (FF11) the duty and
-- the length, NR12 (FF12) the envelope, NR13 and NR14 (FF13, FF14) the
-- frequency, the length's enable and the trigger.
--
-- The frequency is a register (acc) that the CPU writes directly, bit by
-- bit, while it writes NR13 and NR14. The sweep shifts a copy of it right,
-- one bit per microsecond, adds or subtracts it, and writes the sum back at
-- each step of its period; a sum above 2047 stops the channel.
--
-- @hw die: dmg-schematics, category apu-ch1 (sheets "Channel 1", "NR10 to
-- NR14" and "Sweep"); checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity apu_ch1 is
	port (
		apu_reset  : in std_ulogic;
		apu_phi    : in std_ulogic;  -- rises at the start of phase A
		clk_t4     : in std_ulogic;
		ajer       : in std_ulogic;  -- 2 MiHz
		ch1_1mhz   : in std_ulogic;
		hz256      : in std_ulogic;  -- the frame sequencer's clocks
		hz128      : in std_ulogic;
		hz128_ch1  : in std_ulogic;
		hz512      : in std_ulogic;
		test       : in std_ulogic;  -- NR52 bit 4, test mode

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		active : out std_ulogic;  -- NR52 bit 0
		dac_on : out std_ulogic;
		output : out std_ulogic_vector(3 downto 0)
	);
end entity;

architecture rtl of apu_ch1 is
	signal write_nr10, write_nr11, write_nr12, write_nr13, write_nr14 : std_ulogic;
	signal write_nr14_late                                            : std_ulogic;

	signal sweep        : std_ulogic_vector(6 downto 0) := (others => '0');  -- NR10
	signal duty         : std_ulogic_vector(1 downto 0) := "00";  -- NR11 bits 6, 7
	signal envelope     : std_ulogic_vector(7 downto 0) := (others => '0');  -- NR12
	signal acc          : std_ulogic_vector(10 downto 0) := (others => '0');  -- the frequency
	signal length_on    : std_ulogic := '0';  -- NR14 bit 6
	signal trigger      : std_ulogic := '0';  -- NR14 bit 7, until the start
	signal dac_on_i     : std_ulogic;

	-- The trigger
	signal start        : std_ulogic := '0';  -- EZEC
	signal triggered    : std_ulogic := '0';  -- ~FYFO
	signal restart      : std_ulogic := '0';  -- FEKU
	signal restarted    : std_ulogic := '0';  -- FARE
	signal running      : std_ulogic := '0';  -- FYTE
	signal frozen       : std_ulogic := '1';  -- GEXU
	signal active_i     : std_ulogic := '0';  -- CYTO
	signal stop         : std_ulogic;

	-- The frequency and the duty
	signal counter      : unsigned(10 downto 0) := (others => '0');
	signal count_clock  : std_ulogic;
	signal reload       : std_ulogic := '0';  -- COMY
	signal load_counter : std_ulogic;
	signal step         : unsigned(2 downto 0) := (others => '0');
	signal pwm          : std_ulogic;
	signal wave         : std_ulogic := '0';  -- DUWO

	-- The length
	signal length       : unsigned(5 downto 0) := (others => '0');
	signal length_clock : std_ulogic;
	signal expired      : std_ulogic := '0';  -- CERO
	signal length_reset : std_ulogic;

	-- The envelope
	signal hz64         : std_ulogic := '0';  -- KALY
	signal period       : unsigned(2 downto 0) := (others => '0');
	signal load_period  : std_ulogic;
	signal tick         : std_ulogic := '0';  -- KOZY
	signal tick_reset   : std_ulogic;
	signal volume       : unsigned(3 downto 0) := (others => '0');
	signal volume_clock : std_ulogic;
	signal volume_clocks : std_ulogic_vector(3 downto 0);
	signal at_limit     : std_ulogic := '0';  -- KYNO
	signal env_stopped  : std_ulogic := '0';  -- KEZU
	signal no_period    : std_ulogic;

	-- The sweep
	signal sweep_count   : unsigned(2 downto 0) := (others => '0');
	signal load_sweep    : std_ulogic;
	signal sweep_step    : std_ulogic := '0';  -- BEXA
	signal no_sweep      : std_ulogic;
	signal load_shift    : std_ulogic;
	signal shifting      : std_ulogic := '0';  -- ~FEMU
	signal shift_clock   : std_ulogic;
	signal shifts        : unsigned(2 downto 0) := (others => '0');
	signal shifted       : std_ulogic_vector(10 downto 0) := (others => '0');
	signal shift_done    : std_ulogic := '0';  -- BYTE: ld_sum
	signal sum_reset     : std_ulogic;
	signal addend        : std_ulogic_vector(10 downto 0) := (others => '0');
	signal augend        : std_ulogic_vector(10 downto 0) := (others => '0');
	signal sum           : unsigned(11 downto 0);
	signal overflow      : std_ulogic;
	signal update        : std_ulogic;
	signal negate        : std_ulogic;
begin
	-- @hw die: CENU, COVU BORO, GAXU HAFU, DEPU DYLA, DEBY DOGE, BAGE (in G and H)
	write_nr10      <= '1' when cpu_wr = '1' and addr = x"FF10" else '0';
	write_nr11      <= '1' when cpu_wr = '1' and addr = x"FF11" else '0';
	write_nr12      <= '1' when cpu_wr = '1' and addr = x"FF12" else '0';
	write_nr13      <= '1' when cpu_wr = '1' and addr = x"FF13" else '0';
	write_nr14      <= '1' when cpu_wr = '1' and addr = x"FF14" else '0';
	write_nr14_late <= write_nr14 and clk_t4;

	-- The registers are latches, open while the CPU writes them; NR14's bit 6
	-- only in the last two phases of the write.
	-- @hw die: BANY ARAX ANAZ AVAF ADEK BANA BOTU, CENA DYCA, JUSA..JOPU, BOKO
	-- (drlatch_ee)
	registers : process (all)
	begin
		if apu_reset = '1' then
			sweep     <= (others => '0');
			duty      <= "00";
			envelope  <= (others => '0');
			length_on <= '0';
		else
			if write_nr10 = '1' then
				sweep <= to_x01(data(6 downto 0));
			end if;
			if write_nr11 = '1' then
				duty <= to_x01(data(7 downto 6));
			end if;
			if write_nr12 = '1' then
				envelope <= to_x01(data);
			end if;
			if write_nr14_late = '1' then
				length_on <= to_x01(data(6));
			end if;
		end if;
	end process;

	-- @hw die: HOCA
	dac_on_i <= '1' when envelope(7 downto 3) /= "00000" else '0';
	dac_on   <= dac_on_i;

	-- The frequency: flip-flops that the CPU sets and clears directly while
	-- it writes NR13 (bits 0 to 7) and NR14 (bits 8 to 10), and that take the
	-- sweep's sum when it updates.
	-- @hw die: HYKA JYKA HAVO EDUL FELY HOLU HYXU HOPO DYGY EVAB AXAN (dffsr),
	-- and the nand2/nor2 of their set and reset
	frequency : process (write_nr13, write_nr14, data, apu_reset, update)
	begin
		for i in 0 to 10 loop
			if apu_reset = '1' then
				acc(i) <= '0';
			elsif i <= 7 and write_nr13 = '1' then
				acc(i) <= to_x01(data(i));
			elsif i >= 8 and write_nr14 = '1' then
				acc(i) <= to_x01(data(i - 8));
			elsif rising_edge(update) then
				acc(i) <= sum(i) after tco;
			end if;
		end loop;
	end process;

	----------------------------------------------------------------------
	-- The trigger, as channel 2's.
	-- @hw die: DUPE (drlatch_ee), EZEC (dffr); FYFO (nor_latch), FEKU FARE
	-- FYTE (dffr), GEXU (nand_latch); CYTO (nor_latch), BERY
	----------------------------------------------------------------------
	trigger_latch : process (all)
	begin
		if apu_reset = '1' or start = '1' then
			trigger <= '0';
		elsif write_nr14 = '1' then
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

	restart_ff : process (ch1_1mhz, apu_reset, restarted)
	begin
		if apu_reset = '1' then
			restarted <= '0';
			running   <= '0';
		elsif rising_edge(ch1_1mhz) then
			restarted <= restart after tco;
			running   <= restarted after tco;
		end if;
		if apu_reset = '1' or restarted = '1' then
			restart <= '0';
		elsif rising_edge(ch1_1mhz) then
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

	-- The channel stops at the end of its length, when the sweep overflows,
	-- and when its DAC is off.
	stop <= apu_reset or (expired and length_on) or overflow or not dac_on_i;

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
	-- The frequency counter and the duty, as channel 2's.
	-- @hw die: GAXE HYFE JYTY KYNA JEMA HYKE FEVA EKOV EMUS EVAK COPU (tffnl),
	-- FULO GEKU, EPYK FUME DEGA DAKO; COMY (dffr); ESUT (dffr), EROS DAPE
	-- (dffr_cc), DUNA, DUWO (dffr)
	----------------------------------------------------------------------
	count_clock  <= ch1_1mhz or frozen;
	load_counter <= reload or restart;

	frequency_counter : process (load_counter, acc, count_clock)
	begin
		if load_counter = '1' then
			counter <= unsigned(acc);
		elsif falling_edge(count_clock) then
			counter <= counter + 1 after tco;
		end if;
	end process;

	reload_ff : process (counter(10), apu_reset, restart, ch1_1mhz, reload)
	begin
		if apu_reset = '1' or restart = '1' or (reload = '1' and ch1_1mhz = '1') then
			reload <= '0';
		elsif falling_edge(counter(10)) then
			reload <= '1' after tco;
		end if;
	end process;

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

	-- @hw die: COWE BOTO, ACEG AGOF ASON AMOP
	output <= std_ulogic_vector(volume) when (active_i and wave) = '1' or test = '1' else "0000";

	----------------------------------------------------------------------
	-- The length, as channel 2's.
	-- @hw die: BACY CAVY BOVY CUNO CURA ERAM (tffnl), CAPY CANU, BUGY BEPE;
	-- CERO (dffr), CORY
	----------------------------------------------------------------------
	length_clock <= not (length_on and not expired and not hz256);

	length_counter : process (write_nr11, data, length_clock)
	begin
		if write_nr11 = '1' then
			length <= unsigned(to_x01(data(5 downto 0)));
		elsif falling_edge(length_clock) then
			length <= length + 1 after tco;
		end if;
	end process;

	length_reset <= write_nr11 or apu_reset or restart;

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
	-- @hw die: KALY (dffr), JONE; JOVA KENU KERA (tffnl), KAZA KUXU; KOZY
	-- (dffr), KORO KUKU KOMA; HAFO HEMY HOKO HEVO (tffnl), KAKE; KYNO (dffr),
	-- KEZU (nor_latch)
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
	-- The sweep. Its period counts the falls of 128 Hz from NR10's period,
	-- inverted, up to 7: then, at the next rise of 2 MiHz, a step (bexa)
	-- loads the period again and the shifter. A period of 0 never steps.
	-- @hw die: CUPO CYPU CAXY (tffnl), DAFA CYMU; COZE, BEXA (dffr), BURY BAVE
	----------------------------------------------------------------------
	load_sweep <= sweep_step or restart;

	sweep_counter : process (load_sweep, sweep, hz128_ch1)
	begin
		if load_sweep = '1' then
			sweep_count <= unsigned(not to_x01(sweep(6 downto 4)));
		elsif falling_edge(hz128_ch1) then
			sweep_count <= sweep_count + 1 after tco;
		end if;
	end process;

	sweep_step_ff : process (ajer, apu_reset, sweep)
	begin
		if apu_reset = '1' or sweep(6 downto 4) = "000" then
			sweep_step <= '0';
		elsif rising_edge(ajer) then
			sweep_step <= '1' after tco when sweep_count = 7 else '0' after tco;
		end if;
	end process;

	-- The shifter loads the frequency at a step and at the restart; then it
	-- shifts right at each fall of the 1 MiHz clock, NR10's shift times
	-- (counting up from its inverse to 7). Then, at the next rise of 2 MiHz,
	-- the sum's registers take the frequency and the shifted frequency.
	-- @hw die: KALA (ld_shift); FEMU (nand_latch), EVOL EPUK; EGYP DODY EGOR
	-- (shift_clk); COPA CAJA BYRA (tffnl), DACU CYLU; COPY, BYTE (dffr), ATAT;
	-- FABU JEFA GOGA JOLU JOTA FUDE FEDO EXAC ELUX AGEZ BEKU (dffsr)
	no_sweep   <= '1' when sweep(2 downto 0) = "000" else '0';
	load_shift <= sweep_step or restart;

	shifting_latch : process (all)
	begin
		if apu_reset = '1' or shift_done = '1' then
			shifting <= '0';
		elsif sweep_step = '1' or running = '1' then
			shifting <= '1';
		end if;
	end process;

	shift_clock <= no_sweep or (shifting and not ch1_1mhz);

	shift_counter : process (load_shift, sweep, shift_clock)
	begin
		if load_shift = '1' then
			shifts <= unsigned(not to_x01(sweep(2 downto 0)));
		elsif rising_edge(shift_clock) then
			shifts <= shifts + 1 after tco;
		end if;
	end process;

	shift_done_ff : process (ajer, apu_reset, sweep_step)
	begin
		if apu_reset = '1' or sweep_step = '1' then
			shift_done <= '0';
		elsif rising_edge(ajer) then
			shift_done <= '1' after tco when shifts = 7 else '0' after tco;
		end if;
	end process;

	shifter : process (load_shift, acc, shift_clock, apu_reset)
	begin
		if apu_reset = '1' then
			shifted <= (others => '0');
		elsif load_shift = '1' then
			shifted <= acc;
		elsif rising_edge(shift_clock) then
			shifted <= '0' & shifted(10 downto 1) after tco;
		end if;
	end process;

	-- The sum: the frequency plus the shifted frequency, or minus it (NR10
	-- bit 3: the shifted frequency inverted, plus 1).
	-- @hw die: GALO..DOLY, HORA..DEVA (dffr_cc_q), KABA KYNY; ARYL, the xor;
	-- GUXA HALU JULE JORY HEXO GEVA FEGO ETEK DYXE DULE CORU (full_add)
	negate    <= sweep(3);
	sum_reset <= apu_reset or restart;

	sum_ff : process (shift_done, sum_reset)
	begin
		if sum_reset = '1' then
			augend <= (others => '0');
			addend <= (others => '0');
		elsif rising_edge(shift_done) then
			augend <= acc after tco;
			addend <= shifted xor (10 downto 0 => negate) after tco;
		end if;
	end process;

	sum <= ('0' & unsigned(augend)) + ('0' & unsigned(addend)) + unsigned'(0 => negate);

	-- A sum above 2047 (adding) stops the channel; otherwise each step
	-- writes it to the frequency.
	-- @hw die: BYLE ATYS BONE; ATUV BUSO BOJE (ch1_freq_upd)
	overflow <= sum(11) and not negate;
	update   <= sweep_step and not overflow and not no_sweep;

	----------------------------------------------------------------------
	-- Reads: NR10, NR11's duty, NR12, NR14's bit 6. In test mode, NR13 and
	-- NR14 read the frequency counter.
	-- @hw die: ATYN ASOP, BEXU, HAMY HOCU, BALE BYTU; DAXA DYPU EVAJ DUPY
	----------------------------------------------------------------------
	data <= 'Z' & sweep when cpu_rd = '1' and addr = x"FF10" else
	        duty & "ZZZZZZ" when cpu_rd = '1' and addr = x"FF11" else
	        envelope when cpu_rd = '1' and addr = x"FF12" else
	        std_ulogic_vector(counter(7 downto 0)) when cpu_rd = '1' and addr = x"FF13" and test = '1' else
	        'Z' & length_on & "ZZZ" & std_ulogic_vector(counter(10 downto 8))
	            when cpu_rd = '1' and addr = x"FF14" and test = '1' else
	        'Z' & length_on & "ZZZZZZ" when cpu_rd = '1' and addr = x"FF14" else
	        (others => 'Z');
end architecture;
