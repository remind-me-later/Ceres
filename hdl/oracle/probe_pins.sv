// SPDX-License-Identifier: MIT
//
// Writes every change of the pins of dmg-sim's Game Boy to the file given
// by +PINS_FILE=<path>, one per line: "<time in ps> <pin> <value>", with
// the values 0, 1, x and z. Without +PINS_FILE, it does nothing.
//
// Compiled in next to dmg-sim (see the Makefile), it reaches the pins by
// their hierarchical names.
`default_nettype none

module probe_pins;
	integer f = 0;

	initial begin
		string name;
		if ($value$plusargs("PINS_FILE=%s", name))
			f = $fopen(name, "w");
	end

	always @(dmg_cpu_b_gameboy.a[0]) if (f) $fdisplay(f, "%0d a_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[0]);
	always @(dmg_cpu_b_gameboy.a[1]) if (f) $fdisplay(f, "%0d a_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[1]);
	always @(dmg_cpu_b_gameboy.a[2]) if (f) $fdisplay(f, "%0d a_pad2 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[2]);
	always @(dmg_cpu_b_gameboy.a[3]) if (f) $fdisplay(f, "%0d a_pad3 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[3]);
	always @(dmg_cpu_b_gameboy.a[4]) if (f) $fdisplay(f, "%0d a_pad4 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[4]);
	always @(dmg_cpu_b_gameboy.a[5]) if (f) $fdisplay(f, "%0d a_pad5 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[5]);
	always @(dmg_cpu_b_gameboy.a[6]) if (f) $fdisplay(f, "%0d a_pad6 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[6]);
	always @(dmg_cpu_b_gameboy.a[7]) if (f) $fdisplay(f, "%0d a_pad7 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[7]);
	always @(dmg_cpu_b_gameboy.a[8]) if (f) $fdisplay(f, "%0d a_pad8 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[8]);
	always @(dmg_cpu_b_gameboy.a[9]) if (f) $fdisplay(f, "%0d a_pad9 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[9]);
	always @(dmg_cpu_b_gameboy.a[10]) if (f) $fdisplay(f, "%0d a_pad10 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[10]);
	always @(dmg_cpu_b_gameboy.a[11]) if (f) $fdisplay(f, "%0d a_pad11 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[11]);
	always @(dmg_cpu_b_gameboy.a[12]) if (f) $fdisplay(f, "%0d a_pad12 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[12]);
	always @(dmg_cpu_b_gameboy.a[13]) if (f) $fdisplay(f, "%0d a_pad13 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[13]);
	always @(dmg_cpu_b_gameboy.a[14]) if (f) $fdisplay(f, "%0d a_pad14 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[14]);
	always @(dmg_cpu_b_gameboy.a[15]) if (f) $fdisplay(f, "%0d a_pad15 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[15]);
	always @(dmg_cpu_b_gameboy.d[0]) if (f) $fdisplay(f, "%0d d_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[0]);
	always @(dmg_cpu_b_gameboy.d[1]) if (f) $fdisplay(f, "%0d d_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[1]);
	always @(dmg_cpu_b_gameboy.d[2]) if (f) $fdisplay(f, "%0d d_pad2 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[2]);
	always @(dmg_cpu_b_gameboy.d[3]) if (f) $fdisplay(f, "%0d d_pad3 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[3]);
	always @(dmg_cpu_b_gameboy.d[4]) if (f) $fdisplay(f, "%0d d_pad4 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[4]);
	always @(dmg_cpu_b_gameboy.d[5]) if (f) $fdisplay(f, "%0d d_pad5 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[5]);
	always @(dmg_cpu_b_gameboy.d[6]) if (f) $fdisplay(f, "%0d d_pad6 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[6]);
	always @(dmg_cpu_b_gameboy.d[7]) if (f) $fdisplay(f, "%0d d_pad7 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[7]);
	always @(dmg_cpu_b_gameboy.ma[0]) if (f) $fdisplay(f, "%0d ma_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[0]);
	always @(dmg_cpu_b_gameboy.ma[1]) if (f) $fdisplay(f, "%0d ma_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[1]);
	always @(dmg_cpu_b_gameboy.ma[2]) if (f) $fdisplay(f, "%0d ma_pad2 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[2]);
	always @(dmg_cpu_b_gameboy.ma[3]) if (f) $fdisplay(f, "%0d ma_pad3 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[3]);
	always @(dmg_cpu_b_gameboy.ma[4]) if (f) $fdisplay(f, "%0d ma_pad4 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[4]);
	always @(dmg_cpu_b_gameboy.ma[5]) if (f) $fdisplay(f, "%0d ma_pad5 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[5]);
	always @(dmg_cpu_b_gameboy.ma[6]) if (f) $fdisplay(f, "%0d ma_pad6 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[6]);
	always @(dmg_cpu_b_gameboy.ma[7]) if (f) $fdisplay(f, "%0d ma_pad7 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[7]);
	always @(dmg_cpu_b_gameboy.ma[8]) if (f) $fdisplay(f, "%0d ma_pad8 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[8]);
	always @(dmg_cpu_b_gameboy.ma[9]) if (f) $fdisplay(f, "%0d ma_pad9 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[9]);
	always @(dmg_cpu_b_gameboy.ma[10]) if (f) $fdisplay(f, "%0d ma_pad10 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[10]);
	always @(dmg_cpu_b_gameboy.ma[11]) if (f) $fdisplay(f, "%0d ma_pad11 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[11]);
	always @(dmg_cpu_b_gameboy.ma[12]) if (f) $fdisplay(f, "%0d ma_pad12 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[12]);
	always @(dmg_cpu_b_gameboy.md[0]) if (f) $fdisplay(f, "%0d md_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[0]);
	always @(dmg_cpu_b_gameboy.md[1]) if (f) $fdisplay(f, "%0d md_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[1]);
	always @(dmg_cpu_b_gameboy.md[2]) if (f) $fdisplay(f, "%0d md_pad2 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[2]);
	always @(dmg_cpu_b_gameboy.md[3]) if (f) $fdisplay(f, "%0d md_pad3 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[3]);
	always @(dmg_cpu_b_gameboy.md[4]) if (f) $fdisplay(f, "%0d md_pad4 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[4]);
	always @(dmg_cpu_b_gameboy.md[5]) if (f) $fdisplay(f, "%0d md_pad5 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[5]);
	always @(dmg_cpu_b_gameboy.md[6]) if (f) $fdisplay(f, "%0d md_pad6 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[6]);
	always @(dmg_cpu_b_gameboy.md[7]) if (f) $fdisplay(f, "%0d md_pad7 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[7]);
	always @(dmg_cpu_b_gameboy.ld[0]) if (f) $fdisplay(f, "%0d ld_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ld[0]);
	always @(dmg_cpu_b_gameboy.ld[1]) if (f) $fdisplay(f, "%0d ld_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ld[1]);
	always @(dmg_cpu_b_gameboy.phi) if (f) $fdisplay(f, "%0d phi_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.phi);
	always @(dmg_cpu_b_gameboy.nrd) if (f) $fdisplay(f, "%0d rd_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nrd);
	always @(dmg_cpu_b_gameboy.nwr) if (f) $fdisplay(f, "%0d wr_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nwr);
	always @(dmg_cpu_b_gameboy.ncs) if (f) $fdisplay(f, "%0d cs_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ncs);
	always @(dmg_cpu_b_gameboy.nmrd) if (f) $fdisplay(f, "%0d mrd_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nmrd);
	always @(dmg_cpu_b_gameboy.nmwr) if (f) $fdisplay(f, "%0d mwr_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nmwr);
	always @(dmg_cpu_b_gameboy.nmcs) if (f) $fdisplay(f, "%0d mcs_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nmcs);
	always @(dmg_cpu_b_gameboy.cpg) if (f) $fdisplay(f, "%0d cpg_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.cpg);
	always @(dmg_cpu_b_gameboy.cp) if (f) $fdisplay(f, "%0d cp_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.cp);
	always @(dmg_cpu_b_gameboy.cpl) if (f) $fdisplay(f, "%0d cpl_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.cpl);
	always @(dmg_cpu_b_gameboy.fr) if (f) $fdisplay(f, "%0d fr_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.fr);
	always @(dmg_cpu_b_gameboy.st) if (f) $fdisplay(f, "%0d st_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.st);
	always @(dmg_cpu_b_gameboy.s) if (f) $fdisplay(f, "%0d s_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.s);
	always @(dmg_cpu_b_gameboy.dmg.m1_pad) if (f) $fdisplay(f, "%0d m1_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.dmg.m1_pad);
	always @(dmg_cpu_b_gameboy.sout) if (f) $fdisplay(f, "%0d sout_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.sout);
endmodule

`default_nettype wire
