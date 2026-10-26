-- SPDX-License-Identifier: MIT
--
-- The SM83's interrupt logic: IE (FFFF), the requests that are enabled,
-- their priority, the acknowledges to IF, and the vector of the interrupt's
-- entry. It also decodes the addresses FFxx and FFFF.
--
-- The requests (IE and IF) are latched in the first half of the M-cycle
-- (data_phase low); in write_phase (G and H), the lowest one that is
-- requested is selected. The interrupt's entry, in its M-cycle 6 (when it
-- writes PC to the stack and loads PC), acknowledges it and loads its
-- vector, 40 + 8 times its number.
--
-- @hw die: dmg-schematics, categories irq and irq-ie

library ieee;
use ieee.std_logic_1164.all;

use work.clocking.all;
use work.sm83_decoder.all;
use work.sm83_controls.all;

entity sm83_interrupts is
	port (
		data_phase  : in std_ulogic;  -- E to H
		write_phase : in std_ulogic;  -- G and H
		sys_reset   : in std_ulogic;
		s           : in steps_t;
		c           : in controls_t;
		rd          : in std_ulogic;  -- the core reads
		a           : in std_ulogic_vector(15 downto 0);  -- the core's address
		db          : inout std_logic_vector(7 downto 0);  -- the core's data bus
		int         : in std_ulogic_vector(7 downto 0);  -- the requests (IF)
		nmi_entry   : in std_ulogic;

		ffxx        : out std_ulogic;
		ffff        : out std_ulogic;
		int_pending : out std_ulogic;  -- an enabled interrupt is requested
		int_now     : out std_ulogic;  -- ... before the latch (for the CGB's HALT)
		int_take    : out std_ulogic;  -- one is selected (in write_phase)
		inta        : out std_ulogic_vector(7 downto 0);  -- the acknowledges
		int_vector  : out std_ulogic_vector(7 downto 3)   -- the vector's bits
	);
end entity;

architecture rtl of sm83_interrupts is
	signal ffxx_i, ffff_i : std_ulogic;
	signal ie_we          : std_ulogic;
	signal ie_next        : std_ulogic_vector(7 downto 0) := (others => '0');
	signal ie             : std_ulogic_vector(7 downto 0) := (others => '0');
	signal requested      : std_ulogic_vector(7 downto 0);
	signal pending        : std_ulogic_vector(7 downto 0) := (others => '0');
	signal selected       : std_ulogic_vector(7 downto 0);
	signal inta_i         : std_ulogic_vector(7 downto 0);
begin
	-- The addresses: the chain of NANDs of the priority cells.
	-- @hw die: IRQ_PRIO (dist_nand), IRQ_HEAD
	ffxx_i <= '1' when a(15 downto 8) = x"FF" else '0';
	ffff_i <= '1' when ffxx_i = '1' and a(7 downto 0) = x"FF" else '0';
	ffxx   <= ffxx_i;
	ffff   <= ffff_i;

	----------------------------------------------------------------------
	-- IE: all 8 bits are kept. It takes the data bus in write_phase when
	-- the core writes FFFF; the reset clears it.
	-- @hw die: REG_IE (dffr_cc_ee_reg_ie_bit), IRQ_AND, REG_IE_NOT_WE;
	-- IRQ_NAND2_NAND3 (the read)
	----------------------------------------------------------------------
	ie_we <= ffff_i and c.mwrite;

	ie_capture : process (all)
	begin
		if sys_reset = '1' then
			ie_next <= (others => '0');
		elsif ie_we = '1' and write_phase = '1' then
			ie_next <= to_x01(db);
		end if;
	end process;

	-- The output follows while IE is not written.
	ie_update : process (all)
	begin
		if ie_we = '0' then
			ie <= ie_next;
		end if;
	end process;

	-- A read of FFFF pulls the data bus's bits down where IE is 0.
	ie_read : for i in 0 to 7 generate
		db(i) <= '0' when rd = '1' and ffff_i = '1' and ie(i) = '0' else 'Z';
	end generate;

	----------------------------------------------------------------------
	-- The requests are latched while data_phase is low. In write_phase,
	-- the lowest one is selected (none during an NMI's entry); M-cycle 6
	-- of the entry acknowledges it.
	-- @hw die: IRQ_NAND2_NAND3, IRQ_LATCH (dlatch_ee_irq), IRQ_PRIO
	-- (nand_a, nand_b, dist_nor, and, nand_c to nand_e)
	----------------------------------------------------------------------
	requested <= ie and int;

	request_latch : process (all)
	begin
		if data_phase = '0' then
			pending <= requested;
		end if;
	end process;

	int_pending <= '1' when pending /= x"00" else '0';
	int_now     <= '1' when requested /= x"00" else '0';

	priority : process (all)
		variable lower : std_ulogic;  -- a lower one is requested
	begin
		lower := '0';
		for i in 0 to 7 loop
			selected(i) <= write_phase and pending(i) and not lower and not nmi_entry;
			lower := lower or pending(i);
		end loop;
	end process;

	int_take <= '1' when selected /= x"00" else '0';

	inta_i <= selected when s.int_entry_m6 = '1' else x"00";
	inta   <= inta_i;

	-- The vector: 40 + 8 * the interrupt's number, or 80 for an NMI.
	int_vector(3) <= inta_i(1) or inta_i(3) or inta_i(5) or inta_i(7);
	int_vector(4) <= inta_i(2) or inta_i(3) or inta_i(6) or inta_i(7);
	int_vector(5) <= inta_i(4) or inta_i(5) or inta_i(6) or inta_i(7);
	int_vector(6) <= s.int_entry_m6 and write_phase when selected /= x"00" else '0';
	int_vector(7) <= s.int_entry_m6 and nmi_entry;
end architecture;
