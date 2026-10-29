-- SPDX-License-Identifier: MIT
--
-- The chips the model tells apart: the DMG-CPU B (dmg_cpu_b.vhd) and two
-- revisions of the CGB's chip (cgb_cpu.vhd). The blocks they share take
-- the chip as a generic where they differ; each does what the DMG-CPU B's
-- netlist does with DMG_B, and what ../cgb.md lists with the others.

package revisions is
	type revision_t is (DMG_B, CGB_C, CGB_E);
end package;
