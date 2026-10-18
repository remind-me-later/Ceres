// Writes every change of the pins of dmg-sim's Game Boy to pins.trace, in
// the format of the VHDL testbench's trace (see Makefile).
`default_nettype none

module probe_pins;
	integer f;
	initial f = $fopen("pins.trace", "w");
	always @(dmg_cpu_b_gameboy.a[0]) $fdisplay(f, "%0d a_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[0]);
	always @(dmg_cpu_b_gameboy.a[1]) $fdisplay(f, "%0d a_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[1]);
	always @(dmg_cpu_b_gameboy.a[2]) $fdisplay(f, "%0d a_pad2 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[2]);
	always @(dmg_cpu_b_gameboy.a[3]) $fdisplay(f, "%0d a_pad3 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[3]);
	always @(dmg_cpu_b_gameboy.a[4]) $fdisplay(f, "%0d a_pad4 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[4]);
	always @(dmg_cpu_b_gameboy.a[5]) $fdisplay(f, "%0d a_pad5 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[5]);
	always @(dmg_cpu_b_gameboy.a[6]) $fdisplay(f, "%0d a_pad6 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[6]);
	always @(dmg_cpu_b_gameboy.a[7]) $fdisplay(f, "%0d a_pad7 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[7]);
	always @(dmg_cpu_b_gameboy.a[8]) $fdisplay(f, "%0d a_pad8 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[8]);
	always @(dmg_cpu_b_gameboy.a[9]) $fdisplay(f, "%0d a_pad9 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[9]);
	always @(dmg_cpu_b_gameboy.a[10]) $fdisplay(f, "%0d a_pad10 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[10]);
	always @(dmg_cpu_b_gameboy.a[11]) $fdisplay(f, "%0d a_pad11 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[11]);
	always @(dmg_cpu_b_gameboy.a[12]) $fdisplay(f, "%0d a_pad12 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[12]);
	always @(dmg_cpu_b_gameboy.a[13]) $fdisplay(f, "%0d a_pad13 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[13]);
	always @(dmg_cpu_b_gameboy.a[14]) $fdisplay(f, "%0d a_pad14 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[14]);
	always @(dmg_cpu_b_gameboy.a[15]) $fdisplay(f, "%0d a_pad15 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.a[15]);
	always @(dmg_cpu_b_gameboy.d[0]) $fdisplay(f, "%0d d_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[0]);
	always @(dmg_cpu_b_gameboy.d[1]) $fdisplay(f, "%0d d_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[1]);
	always @(dmg_cpu_b_gameboy.d[2]) $fdisplay(f, "%0d d_pad2 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[2]);
	always @(dmg_cpu_b_gameboy.d[3]) $fdisplay(f, "%0d d_pad3 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[3]);
	always @(dmg_cpu_b_gameboy.d[4]) $fdisplay(f, "%0d d_pad4 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[4]);
	always @(dmg_cpu_b_gameboy.d[5]) $fdisplay(f, "%0d d_pad5 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[5]);
	always @(dmg_cpu_b_gameboy.d[6]) $fdisplay(f, "%0d d_pad6 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[6]);
	always @(dmg_cpu_b_gameboy.d[7]) $fdisplay(f, "%0d d_pad7 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.d[7]);
	always @(dmg_cpu_b_gameboy.ma[0]) $fdisplay(f, "%0d ma_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[0]);
	always @(dmg_cpu_b_gameboy.ma[1]) $fdisplay(f, "%0d ma_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[1]);
	always @(dmg_cpu_b_gameboy.ma[2]) $fdisplay(f, "%0d ma_pad2 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[2]);
	always @(dmg_cpu_b_gameboy.ma[3]) $fdisplay(f, "%0d ma_pad3 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[3]);
	always @(dmg_cpu_b_gameboy.ma[4]) $fdisplay(f, "%0d ma_pad4 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[4]);
	always @(dmg_cpu_b_gameboy.ma[5]) $fdisplay(f, "%0d ma_pad5 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[5]);
	always @(dmg_cpu_b_gameboy.ma[6]) $fdisplay(f, "%0d ma_pad6 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[6]);
	always @(dmg_cpu_b_gameboy.ma[7]) $fdisplay(f, "%0d ma_pad7 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[7]);
	always @(dmg_cpu_b_gameboy.ma[8]) $fdisplay(f, "%0d ma_pad8 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[8]);
	always @(dmg_cpu_b_gameboy.ma[9]) $fdisplay(f, "%0d ma_pad9 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[9]);
	always @(dmg_cpu_b_gameboy.ma[10]) $fdisplay(f, "%0d ma_pad10 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[10]);
	always @(dmg_cpu_b_gameboy.ma[11]) $fdisplay(f, "%0d ma_pad11 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[11]);
	always @(dmg_cpu_b_gameboy.ma[12]) $fdisplay(f, "%0d ma_pad12 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ma[12]);
	always @(dmg_cpu_b_gameboy.md[0]) $fdisplay(f, "%0d md_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[0]);
	always @(dmg_cpu_b_gameboy.md[1]) $fdisplay(f, "%0d md_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[1]);
	always @(dmg_cpu_b_gameboy.md[2]) $fdisplay(f, "%0d md_pad2 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[2]);
	always @(dmg_cpu_b_gameboy.md[3]) $fdisplay(f, "%0d md_pad3 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[3]);
	always @(dmg_cpu_b_gameboy.md[4]) $fdisplay(f, "%0d md_pad4 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[4]);
	always @(dmg_cpu_b_gameboy.md[5]) $fdisplay(f, "%0d md_pad5 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[5]);
	always @(dmg_cpu_b_gameboy.md[6]) $fdisplay(f, "%0d md_pad6 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[6]);
	always @(dmg_cpu_b_gameboy.md[7]) $fdisplay(f, "%0d md_pad7 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.md[7]);
	always @(dmg_cpu_b_gameboy.ld[0]) $fdisplay(f, "%0d ld_pad0 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ld[0]);
	always @(dmg_cpu_b_gameboy.ld[1]) $fdisplay(f, "%0d ld_pad1 %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ld[1]);
	always @(dmg_cpu_b_gameboy.phi) $fdisplay(f, "%0d phi_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.phi);
	always @(dmg_cpu_b_gameboy.nrd) $fdisplay(f, "%0d rd_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nrd);
	always @(dmg_cpu_b_gameboy.nwr) $fdisplay(f, "%0d wr_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nwr);
	always @(dmg_cpu_b_gameboy.ncs) $fdisplay(f, "%0d cs_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.ncs);
	always @(dmg_cpu_b_gameboy.nmrd) $fdisplay(f, "%0d mrd_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nmrd);
	always @(dmg_cpu_b_gameboy.nmwr) $fdisplay(f, "%0d mwr_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nmwr);
	always @(dmg_cpu_b_gameboy.nmcs) $fdisplay(f, "%0d mcs_n_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.nmcs);
	always @(dmg_cpu_b_gameboy.cpg) $fdisplay(f, "%0d cpg_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.cpg);
	always @(dmg_cpu_b_gameboy.cp) $fdisplay(f, "%0d cp_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.cp);
	always @(dmg_cpu_b_gameboy.cpl) $fdisplay(f, "%0d cpl_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.cpl);
	always @(dmg_cpu_b_gameboy.fr) $fdisplay(f, "%0d fr_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.fr);
	always @(dmg_cpu_b_gameboy.st) $fdisplay(f, "%0d st_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.st);
	always @(dmg_cpu_b_gameboy.s) $fdisplay(f, "%0d s_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.s);
	always @(dmg_cpu_b_gameboy.dmg.m1_pad) $fdisplay(f, "%0d m1_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.dmg.m1_pad);
	always @(dmg_cpu_b_gameboy.sout) $fdisplay(f, "%0d sout_pad %b", longint'($realtime * 1000.0), dmg_cpu_b_gameboy.sout);
endmodule

`default_nettype wire
