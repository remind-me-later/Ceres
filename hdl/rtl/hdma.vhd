-- SPDX-License-Identifier: MIT
--
-- The CGB's VRAM DMA (HDMA1 to HDMA5, FF51 to FF55): it copies blocks of 16
-- bytes from the cartridge's bus (ROM, its RAM, the work RAM) to VRAM, in
-- the bank VBK selects, all at once (general-purpose) or a block per
-- HBlank.
--
-- A transfer runs after an opcode fetch, while the CPU core waits: the
-- core's clock is held for whole M-cycles (../clocks.vhd), and the M-cycle
-- after the fetch then follows. At normal speed a transfer of n blocks
-- holds the core 8n + 1 M-cycles: half an M-cycle to begin, a byte every
-- half M-cycle (2 T-cycles), half an M-cycle to end. In double speed it
-- holds it 16n + 1: an M-cycle to begin, a byte an M-cycle. Each byte is
-- read from the cartridge's bus (as OAM DMA reads it) in its slot, and
-- written to VRAM in the slot's second half.
--
-- An HBlank transfer copies a block once one is requested: when mode 0
-- begins while the CPU is not halted, when the transfer starts in mode 0,
-- and when the LCD is turned off. Writing HDMA5 with bit 7 clear stops it,
-- but a requested block still runs, as a general-purpose transfer of the
-- blocks left.
--
-- @hw inferred: no netlist of the CGB's chip is known. The registers are
-- Pan Docs' ("VRAM DMA Transfers"), the timing and the rules ceres's (from
-- SameBoy's GB_hdma_run). Not modelled: the transfer while the CPU is
-- halted or switches speed, writes into both banks while the PPU blocks
-- VRAM, clashes with the PPU's or OAM DMA's accesses, OAM written by HDMA.
-- How the transfer holds the CPU core is a guess.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity hdma is
	port (
		clk          : in std_ulogic;  -- the CPU's clock
		phase        : in phase_t;     -- the phase of the CPU's M-cycle (not the core's)
		reset2       : in std_ulogic;
		cgb_mode     : in std_ulogic;
		double_speed : in std_ulogic;

		-- The CPU's bus
		addr     : in  std_ulogic_vector(15 downto 0);
		data     : in  std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd   : in  std_ulogic;
		cpu_wr   : in  std_ulogic;

		-- The CPU core
		fetch   : in std_ulogic;  -- it fetches an opcode
		running : in std_ulogic;  -- it is not halted

		-- The PPU
		mode0  : in std_ulogic;  -- HBlank
		lcd_on : in std_ulogic;

		hold : out std_ulogic;  -- the CPU core waits

		-- The cartridge's bus
		bus_on   : out std_ulogic;  -- HDMA drives the address pins
		bus_addr : out std_ulogic_vector(15 downto 0);
		d_pins   : in  std_logic_vector(7 downto 0);

		-- VRAM
		vram_write : out std_ulogic;
		vram_addr  : out std_ulogic_vector(12 downto 0);
		vram_data  : out std_ulogic_vector(7 downto 0)
	);
end entity;

architecture rtl of hdma is
	type transfer_t is (IDLE, GENERAL, HBLANK);
	type step_t is (START, BYTES, FINISH);  -- the burst's

	signal src, dst      : unsigned(15 downto 0) := (others => '0');
	signal blocks        : unsigned(7 downto 0) := (others => '0');  -- left, 1 to 128
	signal transfer      : transfer_t := IDLE;
	signal requested     : std_ulogic := '0';
	signal write_5       : std_ulogic;
	signal write_1, write_2, write_3, write_4 : std_ulogic;
	signal hdma_regs     : std_ulogic;

	-- The burst
	signal fetched       : std_ulogic := '0';  -- the M-cycle that ends fetched an opcode
	signal hold_i        : std_ulogic := '0';
	signal step          : step_t := START;
	signal byte_slot     : std_ulogic;
	signal idle_bit      : std_ulogic;
	signal second_half   : std_ulogic;
	signal mode0_was     : std_ulogic := '0';
	signal lcd_was       : std_ulogic := '0';
	signal due           : std_ulogic;
	signal valid_source  : std_ulogic;
begin
	hdma_regs <= cgb_mode when addr(15 downto 3) = x"FF5" & '0' and addr(2 downto 0) /= "000" else '0';
	write_1   <= cpu_wr and hdma_regs when addr(2 downto 0) = "001" else '0';
	write_2   <= cpu_wr and hdma_regs when addr(2 downto 0) = "010" else '0';
	write_3   <= cpu_wr and hdma_regs when addr(2 downto 0) = "011" else '0';
	write_4   <= cpu_wr and hdma_regs when addr(2 downto 0) = "100" else '0';
	write_5   <= cpu_wr and hdma_regs when addr(2 downto 0) = "101" else '0';

	due <= '1' when transfer = GENERAL or (transfer = HBLANK and requested = '1') else '0';

	-- The registers (written as the write strobe falls) and the transfer,
	-- which counts slots of half an M-cycle (an M-cycle in double speed):
	-- they end at the starts of phases A and E (A).
	control : process (clk, write_1, write_2, write_3, write_4, write_5, reset2)
		variable next_dst : unsigned(15 downto 0);
		variable slot_end : boolean;
	begin
		if reset2 = '1' then
			src       <= (others => '0');
			dst       <= (others => '0');
			blocks    <= (others => '0');
			transfer  <= IDLE;
			requested <= '0';
			hold_i    <= '0';
			step      <= START;
			fetched   <= '0';
		elsif falling_edge(write_1) then
			src(15 downto 8) <= unsigned(to_x01(data));
			if unsigned(to_x01(data)) >= x"E0" then
				src(15 downto 12) <= x"F";
			end if;
		elsif falling_edge(write_2) then
			src(7 downto 0) <= unsigned(to_x01(data(7 downto 4))) & "0000";
		elsif falling_edge(write_3) then
			dst(15 downto 8) <= unsigned(to_x01(data));
		elsif falling_edge(write_4) then
			dst(7 downto 0) <= unsigned(to_x01(data(7 downto 4))) & "0000";
		elsif falling_edge(write_5) then
			blocks <= ('0' & unsigned(to_x01(data(6 downto 0)))) + 1;
			if transfer = HBLANK and to_x01(data(7)) = '0' then
				transfer <= GENERAL when requested = '1' else IDLE;
			elsif to_x01(data(7)) = '1' then
				transfer  <= HBLANK;
				requested <= mode0 and lcd_on;
			else
				transfer <= GENERAL;
			end if;
		elsif clk'event then
			-- HBlank's requests: mode 0 begins with the CPU running, or the
			-- LCD is turned off.
			mode0_was <= mode0;
			lcd_was   <= lcd_on;
			if transfer = HBLANK and hold_i = '0'
				and ((mode0 = '1' and mode0_was = '0' and running = '1') or (lcd_on = '0' and lcd_was = '1')) then
				requested <= '1';
			end if;

			if starts(clk, phase, E) and hold_i = '0' then
				fetched <= fetch and running;
			end if;

			slot_end := starts(clk, phase, A) or (starts(clk, phase, E) and double_speed = '0');
			if hold_i = '0' then
				-- A transfer starts as the M-cycle after an opcode fetch
				-- starts.
				if starts(clk, phase, A) and fetched = '1' and due = '1' then
					hold_i <= '1' after tco;
					step   <= START;
				end if;
			elsif slot_end then
				case step is
					when START =>
						step <= BYTES;
					when BYTES =>
						src      <= src + 1;
						next_dst := dst + 1;
						dst      <= next_dst;
						if next_dst(3 downto 0) = "0000" then
							-- A block ends: the transfer ends with its last block,
							-- or when the destination wraps; an HBlank transfer
							-- waits for the next HBlank.
							blocks <= blocks - 1;
							if blocks = 1 or next_dst = 0 then
								transfer <= IDLE;
							elsif transfer = HBLANK then
								requested <= '0';
							end if;
							if blocks = 1 or next_dst = 0 or transfer = HBLANK then
								if double_speed = '1' then
									hold_i <= '0' after tco;
									step   <= START;
								else
									step <= FINISH;
								end if;
							end if;
						end if;
					when FINISH =>
						hold_i <= '0' after tco;
						step   <= START;
				end case;
			end if;
		end if;
	end process;
	hold <= hold_i;

	-- The byte: from the cartridge's bus, but FF from VRAM's addresses and
	-- from E000 on.
	valid_source <= '0' when src(15 downto 13) = "100" or src(15 downto 13) = "111" else '1';
	byte_slot <= '1' when step = BYTES else '0';
	bus_on   <= hold_i and byte_slot and valid_source;
	bus_addr <= std_ulogic_vector(src);

	-- The slot's second half: phases C, D, G, H (in double speed, E to H).
	second_half <= '1' when (double_speed = '0' and (phase = C or phase = D or phase = G or phase = H))
	                     or (double_speed = '1' and phase >= E) else '0';
	vram_write <= hold_i and byte_slot and second_half;
	vram_addr  <= std_ulogic_vector(dst(12 downto 0));
	vram_data  <= to_x01(d_pins) when valid_source = '1' else x"FF";

	idle_bit <= '1' when transfer = IDLE else '0';
	data_out <= x"FF" when cpu_rd = '1' and hdma_regs = '1' and addr(2 downto 0) /= "101" else
	            idle_bit & std_logic_vector(blocks(6 downto 0) - 1)
	                when cpu_rd = '1' and hdma_regs = '1' else
	            (others => 'Z');
end architecture;
