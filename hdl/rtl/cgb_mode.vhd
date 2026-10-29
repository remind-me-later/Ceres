-- SPDX-License-Identifier: MIT
--
-- The CGB's mode and its own small registers: KEY0 (FF4C), which the boot
-- ROM writes to choose between the CGB's mode and the DMG's, OPRI (FF6C),
-- which chooses how objects take priority, and the undocumented FF72 to
-- FF75, which do nothing but keep what is written.
--
-- The CGB mode decides whether the CGB's registers answer (VBK, SVBK, KEY1,
-- HDMA, the colour palettes' data) and whether the PPU reads attributes from
-- VRAM's second bank. Both KEY0 and OPRI take writes only while the boot ROM
-- is mapped; afterwards they are locked until the next reset.
--
-- @hw inferred: no netlist of the CGB's chip is known. The behaviour is
-- SameBoy's (Core/memory.c, KEY0, OPRI, PSWX, PSWY, PSW, PGB) and Pan Docs'
-- ("CGB Registers"); the timing of the writes is the DMG's for its own
-- registers (they latch as the write strobe falls, as FF50 does).

library ieee;
use ieee.std_logic_1164.all;

entity cgb_mode is
	port (
		reset2  : in std_ulogic;
		boot_on : in std_ulogic;  -- the boot ROM is mapped

		-- The CPU's bus
		addr     : in  std_ulogic_vector(15 downto 0);
		data     : in  std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd   : in  std_ulogic;
		cpu_wr   : in  std_ulogic;

		cgb_mode     : out std_ulogic;  -- the CGB's mode, not the DMG's
		dmg_priority : out std_ulogic   -- objects take priority by X, as on the DMG
	);
end entity;

architecture rtl of cgb_mode is
	signal write_key0, write_opri               : std_ulogic;
	signal write_ff72, write_ff73, write_ff74   : std_ulogic;
	signal write_ff75                           : std_ulogic;
	signal cgb_mode_i                           : std_ulogic := '1';
	signal opri                                 : std_ulogic := '0';
	signal dmg_priority_i                       : std_ulogic := '0';
	signal ff72, ff73, ff74                     : std_ulogic_vector(7 downto 0) := (others => '0');
	signal ff75                                 : std_ulogic_vector(6 downto 4) := (others => '0');
begin
	write_key0 <= '1' when cpu_wr = '1' and addr = x"FF4C" else '0';
	write_opri <= '1' when cpu_wr = '1' and addr = x"FF6C" else '0';
	write_ff72 <= '1' when cpu_wr = '1' and addr = x"FF72" else '0';
	write_ff73 <= '1' when cpu_wr = '1' and addr = x"FF73" else '0';
	write_ff74 <= '1' when cpu_wr = '1' and addr = x"FF74" else '0';
	write_ff75 <= '1' when cpu_wr = '1' and addr = x"FF75" else '0';

	-- KEY0: bit 2 chooses the DMG's mode (the boot ROM writes 04 for a
	-- cartridge without the CGB flag), and so does bit 3 (the "PGB" mode,
	-- which nobody has described). The CGB's mode at power-on.
	-- @hw inferred: SameBoy (cgb_mode = !(value & 0xC)); Pan Docs
	-- @rule cgb-mode-registers
	key0 : process (write_key0, reset2)
	begin
		if reset2 = '1' then
			cgb_mode_i <= '1';
		elsif falling_edge(write_key0) and boot_on = '1' then
			cgb_mode_i <= not (to_x01(data(2)) or to_x01(data(3)));
		end if;
	end process;
	cgb_mode <= cgb_mode_i;

	-- OPRI: the boot ROM writes 1 for a cartridge without the CGB flag, so
	-- that its objects take priority by X. Afterwards the register still
	-- keeps a write in the CGB's mode, but the priority stays.
	-- @hw inferred: SameBoy (GB_IO_OPRI); Pan Docs ("does not have an
	-- effect ... after the boot ROM has been unmapped", to be verified)
	-- @rule cgb-mode-registers
	opri_reg : process (write_opri, reset2)
	begin
		if reset2 = '1' then
			opri           <= '0';
			dmg_priority_i <= '0';
		elsif falling_edge(write_opri) then
			if boot_on = '1' then
				opri           <= to_x01(data(0));
				dmg_priority_i <= to_x01(data(0));
			elsif cgb_mode_i = '1' then
				opri <= to_x01(data(0));
			end if;
		end if;
	end process;
	dmg_priority <= dmg_priority_i;

	-- FF72 and FF73 keep any byte; FF74 too, in the CGB's mode only (it
	-- reads FF in the DMG's); FF75 keeps bits 4 to 6.
	-- @hw inferred: Pan Docs ("FF72-FF73", "FF74", "FF75"); SameBoy (PSWX,
	-- PSWY on any CGB, PSW in the CGB's mode)
	-- @rule cgb-mode-registers
	undocumented : process (write_ff72, write_ff73, write_ff74, write_ff75, reset2)
	begin
		if reset2 = '1' then
			ff72 <= (others => '0');
			ff73 <= (others => '0');
			ff74 <= (others => '0');
			ff75 <= (others => '0');
		else
			if falling_edge(write_ff72) then
				ff72 <= to_x01(data);
			end if;
			if falling_edge(write_ff73) then
				ff73 <= to_x01(data);
			end if;
			if falling_edge(write_ff74) and cgb_mode_i = '1' then
				ff74 <= to_x01(data);
			end if;
			if falling_edge(write_ff75) then
				ff75 <= to_x01(data(6 downto 4));
			end if;
		end if;
	end process;

	-- KEY0 does not read back.
	data_out <= "1111111" & opri when cpu_rd = '1' and addr = x"FF6C" else
	            ff72 when cpu_rd = '1' and addr = x"FF72" else
	            ff73 when cpu_rd = '1' and addr = x"FF73" else
	            ff74 when cpu_rd = '1' and addr = x"FF74" and cgb_mode_i = '1' else
	            '1' & ff75 & "1111" when cpu_rd = '1' and addr = x"FF75" else
	            (others => 'Z');
end architecture;
