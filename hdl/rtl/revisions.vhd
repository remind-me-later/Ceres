-- SPDX-License-Identifier: MIT
--
-- The revisions of the CGB's chip that the model tells apart. Where they
-- differ, the blocks take the revision as a generic.

package revisions is
	type revision_t is (CGB_C, CGB_E);
end package;
