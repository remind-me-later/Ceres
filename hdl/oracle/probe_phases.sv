// SPDX-License-Identifier: MIT
//
// Samples signals of dmg-sim once per phase and writes those that changed
// to the file given by +PHASES_FILE=<path>, one per line:
//
//   <phase> <name> <value>
//
// A phase is half a period of the crystal: dmg-sim's testbench toggles the
// crystal pin every 119 ns, and phase k lasts from k * 119 ns to
// (k + 1) * 119 ns. A signal is sampled at the end of its phase, 1 ps
// before the crystal toggles, when every change that the previous edge
// caused has settled. The values are in binary, with x and z, except for
// "phase", which is the letter A to H.
//
// The readable model in ../rtl is checked against these samples (see
// ../check). Each signal here has the name the model gives it, and the
// expression that computes it from the nets of the netlist, which keep
// the names of the cells that drive them (see dmg-schematics).
//
// Without +PHASES_FILE, the probe does nothing.
`default_nettype none

module probe_phases;
	integer f = 0;
	longint phase = -1;
	event tick;

	localparam time T_phase = 119ns;

	initial begin
		string name;
		if ($value$plusargs("PHASES_FILE=%s", name)) begin
			f = $fopen(name, "w");
			#(T_phase - 1ps);
			forever begin
				phase++;
				-> tick;
				#T_phase;
			end
		end
	end

`define CHIP dmg_cpu_b_gameboy.dmg

	// Writes the signal in the first phase, and in every phase in which it
	// changed. A process per signal waits for a change before it waits for
	// the end of the phase, so the signals that do not change cost nothing.
`define TRACE(name, expr) \
	initial begin : trace_``name \
		logic [$bits(expr) - 1:0] last; \
		@(tick); \
		forever begin \
			last = (expr); \
			$fdisplay(f, "%0d %s %b", phase, `"name`", last); \
			do begin \
				@(expr); \
				@(tick); \
			end while ((expr) === last); \
		end \
	end

	// The phase of the M-cycle, from the four latches that count them.
	function automatic string phase_letter(input logic afur, alef, apuk, adyk);
		case ({afur, alef, apuk, adyk})
			4'b1000: return "A";
			4'b1100: return "B";
			4'b1110: return "C";
			4'b1111: return "D";
			4'b0111: return "E";
			4'b0011: return "F";
			4'b0001: return "G";
			4'b0000: return "H";
			default: return "?";
		endcase
	endfunction

	initial begin : trace_phase
		string last;
		@(tick);
		forever begin
			last = phase_letter(`CHIP.afur, `CHIP.alef, `CHIP.apuk, `CHIP.adyk);
			$fdisplay(f, "%0d phase %s", phase, last);
			do begin
				@(`CHIP.afur, `CHIP.alef, `CHIP.apuk, `CHIP.adyk);
				@(tick);
			end while (phase_letter(`CHIP.afur, `CHIP.alef, `CHIP.apuk, `CHIP.adyk) == last);
		end
	end

	// Pins
	`TRACE(ck1, dmg_cpu_b_gameboy.xi)
	`TRACE(reset_n, dmg_cpu_b_gameboy.nreset)
	`TRACE(t1, `CHIP.t1_pad)
	`TRACE(t2, `CHIP.t2_pad)
	`TRACE(phi, `CHIP.phi_pad)

	// From the CPU core
	`TRACE(osc_ena, `CHIP.osc_ena)
	`TRACE(clk_ena, `CHIP.clk_ena)
	`TRACE(mreq, `CHIP.mreq)
	`TRACE(write, `CHIP.write)
	`TRACE(ctl_mreq, `CHIP.cpu_inst.ctl_mreq)

	// The CPU's bus inside the chip
	`TRACE(addr, {`CHIP.a15, `CHIP.a14, `CHIP.a13, `CHIP.a12, `CHIP.a11, `CHIP.a10, `CHIP.a9, `CHIP.a8,
	              `CHIP.a7, `CHIP.a6, `CHIP.a5, `CHIP.a4, `CHIP.a3, `CHIP.a2, `CHIP.a1, `CHIP.a0})
	`TRACE(data, {`CHIP.d7, `CHIP.d6, `CHIP.d5, `CHIP.d4, `CHIP.d3, `CHIP.d2, `CHIP.d1, `CHIP.d0})
	`TRACE(cpu_rd, `CHIP.cpu_rd)
	`TRACE(cpu_wr, `CHIP.cpu_wr)

	// Test register FF60
	`TRACE(test_reset, !`CHIP.test_reset_n)
	`TRACE(ff60_d1, `CHIP.ff60_d1)

	// Clocks and resets
	`TRACE(clk, `CHIP.atal_4mhz)
	`TRACE(clk_1mhz, `CHIP.clk_1mhz)
	`TRACE(clk_t4, `CHIP.clk_t4)
	`TRACE(data_phase, `CHIP.data_phase)
	`TRACE(exec_phase, `CHIP.exec_phase)
	`TRACE(pch_phase, !`CHIP.pch_phase_n)
	`TRACE(cpu_wr_sync, `CHIP.cpu_wr_sync)
	`TRACE(cpu_ext_cs_en, `CHIP.cpu_ext_cs_en)
	`TRACE(reset, `CHIP.reset)
	`TRACE(osc_stable, `CHIP.osc_stable)
	`TRACE(sys_reset, `CHIP.asol_n)
	`TRACE(cpu_sys_reset, `CHIP.cpu_sys_reset)
	`TRACE(reset2, !`CHIP.reset2_n)

	// The divider: DIV (FF04) is bits 6 to 13
	`TRACE(div_read, `CHIP.tagy)
	`TRACE(div, {`CHIP.\16hz , !`CHIP.uket_n, `CHIP.teka, `CHIP.subu, `CHIP.sola, `CHIP.teru,
	             `CHIP.tofe, `CHIP.tugo, `CHIP.tulu, `CHIP.ugot, `CHIP.tama, !`CHIP.unyk_n,
	             `CHIP.tero, !`CHIP.uner_n, `CHIP.ufor, !`CHIP.ukup_n})

	// The timer
	`TRACE(tima, {`CHIP.nuga, `CHIP.peda, `CHIP.rage, `CHIP.ruby, `CHIP.rate, `CHIP.peru, `CHIP.povy, `CHIP.rega})
	`TRACE(tma, {`CHIP.seta, `CHIP.peto, `CHIP.sufy, `CHIP.tyru, `CHIP.tyva, `CHIP.muru, `CHIP.nyke, `CHIP.sabu})
	`TRACE(tac, {!`CHIP.sabo_n, `CHIP.samy, `CHIP.sopu})
	`TRACE(tima_bit7_before, !`CHIP.nydu_n)
	`TRACE(int_timer, `CHIP.int_timer)
	`TRACE(timer_read, `CHIP.teda | `CHIP.tuby | `CHIP.sora)

	// The interrupt flags
	`TRACE(int_requests, {`CHIP.int_jp, `CHIP.int_serial, `CHIP.int_timer, `CHIP.int_stat, `CHIP.int_vbl})
	`TRACE(int_acks, {`CHIP.cpu_irq_ack4, `CHIP.cpu_irq_ack3, `CHIP.cpu_irq_ack2, `CHIP.cpu_irq_ack1,
	                  `CHIP.cpu_irq_ack0})
	`TRACE(int_flags, {`CHIP.cpu_irq4, `CHIP.cpu_irq3, `CHIP.cpu_irq2, `CHIP.cpu_irq1, `CHIP.cpu_irq0})
	`TRACE(int_read, `CHIP.pola)

	// The serial port
	`TRACE(sb, {`CHIP.eder, `CHIP.erod, `CHIP.ejab, `CHIP.dovu, `CHIP.dojo, `CHIP.dyra, `CHIP.degu, `CHIP.cuba})
	`TRACE(sc, {!`CHIP.sck_en_n, `CHIP.sck_dir})
	`TRACE(serial_count, {`CHIP.int_serial, !`CHIP.cyde_n, !`CHIP.cylo_n, !`CHIP.cafa_n})
	`TRACE(serial_half, `CHIP.coty)
	`TRACE(ser_out, `CHIP.ser_out)
	`TRACE(serial_read, `CHIP.ufeg | `CHIP.ucom)
	`TRACE(sck, `CHIP.sck_pad)
	`TRACE(sin, `CHIP.sin_pad)
	`TRACE(sout, `CHIP.sout_pad)

	// The joypad
	`TRACE(p1, {`CHIP.keru, `CHIP.kuko, !`CHIP.cofy_n, !`CHIP.kely_n, `CHIP.kyme, `CHIP.jale, `CHIP.kecy, `CHIP.jute})
	`TRACE(p10_13, {`CHIP.p13_pad, `CHIP.p12_pad, `CHIP.p11_pad, `CHIP.p10_pad})
	`TRACE(p14, `CHIP.p14_pad)
	`TRACE(p15, `CHIP.p15_pad)
	`TRACE(joypad_history, {`CHIP.apug, `CHIP.agem, `CHIP.acef, `CHIP.batu})
	`TRACE(int_jp, `CHIP.int_jp)
	`TRACE(cpu_wakeup, `CHIP.cpu_wakeup)
	`TRACE(joypad_read, `CHIP.ff00rd)

	// Test mode
	`TRACE(t1_only, `CHIP.t1t2_n)
	`TRACE(t2_only, `CHIP.t1_nt2)
	`TRACE(ff60, {`CHIP.ff60_d1, `CHIP.ff60_d0})

	// The boot ROM and high RAM
	`TRACE(boot_unmapped, `CHIP.tepu)
	`TRACE(boot_sel, `CHIP.boot_sel)
	`TRACE(boot_read, `CHIP.bootrom_oe_bl_precharge_n | `CHIP.texe)
	`TRACE(hram_read, `CHIP.hram_oe)

	// The cartridge bus
	`TRACE(read, `CHIP.read)
	`TRACE(non_vram_mreq, `CHIP.non_vram_mreq)
	`TRACE(dma_addr_ext, `CHIP.dma_addr_ext)
	`TRACE(dma_addr, {`CHIP.dma_a15, `CHIP.dma_a14, `CHIP.dma_a13, `CHIP.dma_a12, `CHIP.dma_a11, `CHIP.dma_a10,
	                  `CHIP.dma_a9, `CHIP.dma_a8, `CHIP.dma_a7, `CHIP.dma_a6, `CHIP.dma_a5, `CHIP.dma_a4,
	                  `CHIP.dma_a3, `CHIP.dma_a2, `CHIP.dma_a1, `CHIP.dma_a0})
	`TRACE(latched_addr, {`CHIP.nyre, `CHIP.lonu, `CHIP.lobu, `CHIP.lumy, `CHIP.pate, `CHIP.lysa, `CHIP.luno,
	                      `CHIP.arym, `CHIP.aros, `CHIP.atev, `CHIP.avys, `CHIP.aret, `CHIP.alyr, `CHIP.apur,
	                      `CHIP.alor})
	`TRACE(latched_data, ~{`CHIP.sazy, `CHIP.rupa, `CHIP.sago, `CHIP.sody, `CHIP.selo, `CHIP.raxy, `CHIP.rony,
	                       `CHIP.soma})
	`TRACE(pins_out, !`CHIP.roru)
	`TRACE(a_pins, `CHIP.a_pad)
	`TRACE(d_pins, `CHIP.d_pad)
	`TRACE(rd_n, `CHIP.rd_n_pad)
	`TRACE(wr_n, `CHIP.wr_n_pad)
	`TRACE(cs_n, `CHIP.cs_n_pad)

	// OAM DMA
	`TRACE(dma_phi, `CHIP.dma_phi)
	`TRACE(dma_requested, !`CHIP.lyxe_n)
	`TRACE(dma_start, {!`CHIP.lene_n, `CHIP.luvy})
	`TRACE(dma_enabled, `CHIP.loky)
	`TRACE(dma_done, !`CHIP.myte_n)
	`TRACE(dma_run, `CHIP.dma_run)
	`TRACE(vram_to_oam, `CHIP.vram_to_oam)
	`TRACE(dma_read, `CHIP.molu)

	// The PPU: LCDC, and the timing of lines and frames
	`TRACE(lcdc, {`CHIP.ff40_d7, `CHIP.ff40_d6, `CHIP.ff40_d5, `CHIP.ff40_d4, `CHIP.ff40_d3, `CHIP.ff40_d2,
	              `CHIP.ff40_d1, `CHIP.ff40_d0})
	`TRACE(ppu_reset, `CHIP.ppu_reset)
	`TRACE(wuvu, !`CHIP.wuvu_n)
	`TRACE(vena, !`CHIP.vena_n)
	`TRACE(wosu, `CHIP.wosu)
	`TRACE(lx, {`CHIP.lcd_x6, `CHIP.lcd_x5, `CHIP.lcd_x4, `CHIP.lcd_x3, `CHIP.lcd_x2, `CHIP.lcd_x1, `CHIP.lcd_x0})
	`TRACE(vclk, `CHIP.vclk)
	`TRACE(nype, `CHIP.nype)
	`TRACE(mode1, `CHIP.popu)
	`TRACE(ly, {`CHIP.v7, `CHIP.v6, `CHIP.v5, `CHIP.v4, `CHIP.v3, `CHIP.v2, `CHIP.v1, `CHIP.v0})
	`TRACE(line_153, `CHIP.myta)
	`TRACE(first_line, `CHIP.meda)
	`TRACE(frame, `CHIP.napo)
	`TRACE(line_parity, !`CHIP.luca_n)
	`TRACE(cpg_lx, `CHIP.sygu)
	`TRACE(lyc, {`CHIP.raha, `CHIP.vevo, `CHIP.vafa, `CHIP.sota, `CHIP.salo, `CHIP.sedy, `CHIP.vuce, `CHIP.syry})
	`TRACE(lyc_match, `CHIP.lyc_int)
	`TRACE(stat_enable, {`CHIP.lyc_int_en, `CHIP.int_oam_en, `CHIP.int_vbl_en, `CHIP.int_hbl_en})
	`TRACE(int_vbl, `CHIP.int_vbl)
	`TRACE(int_stat, `CHIP.int_stat)
	`TRACE(mode2, `CHIP.mode2)
	`TRACE(mode3, `CHIP.mode3)
	`TRACE(mode0, `CHIP.mode0)
	`TRACE(ppu_read, `CHIP.wyce == 0 || `CHIP.tobe || `CHIP.wafu || `CHIP.xyly)
	`TRACE(cpl, `CHIP.cpl_pad)
	`TRACE(cpg, `CHIP.cpg_pad)
	`TRACE(fr, `CHIP.fr_pad)
	`TRACE(s, `CHIP.s_pad)
	`TRACE(cp, `CHIP.cp_pad)
	`TRACE(st, `CHIP.st_pad)
	`TRACE(ld, `CHIP.ld_pad)

	// The PPU's rendering: the state that the checks of the rendering compare
	`TRACE(px, {`CHIP.h7, `CHIP.h6, `CHIP.h5, `CHIP.h4, `CHIP.h3, `CHIP.h2, `CHIP.h1, `CHIP.h0})
	`TRACE(mode_state, {`CHIP.mode2, `CHIP.mode3, `CHIP.mode0, `CHIP.px9, `CHIP.voga, `CHIP.stop_mode3,
	                    `CHIP.clkpipe, `CHIP.sprite_x_match})
	`TRACE(scan_index, {`CHIP.oam_parse_a7, `CHIP.oam_parse_a6, `CHIP.oam_parse_a5, `CHIP.oam_parse_a4,
	                    `CHIP.oam_parse_a3, `CHIP.oam_parse_a2})
	`TRACE(scan_state, {`CHIP.start_oam_parsing, `CHIP.anel, `CHIP.oam_parsing, !`CHIP.ceno_n, `CHIP.byba,
	                    `CHIP.doba, `CHIP.stop_oam_parsing})
	`TRACE(sprite_count, {`CHIP.dybe, `CHIP.bego, `CHIP.cuxy, `CHIP.bese})
	`TRACE(sprite_save, {`CHIP.dezy, !`CHIP.sprite_save_en_n, `CHIP.sprite_y_match})
	`TRACE(fetch, {`CHIP.laxu, `CHIP.mesu, `CHIP.nyva, `CHIP.lyzu, !`CHIP.lovy_n, `CHIP.lony, `CHIP.nyka,
	               `CHIP.pory, `CHIP.pygo, `CHIP.poky})
	`TRACE(window_state, {!`CHIP.nopa_n, `CHIP.nunu, `CHIP.pynu, `CHIP.pyco, `CHIP.sovy, `CHIP.wy_latch,
	                      `CHIP.sary, `CHIP.wy_match, `CHIP.wxy_match})
	`TRACE(fine, {`CHIP.rene, `CHIP.ryfa, `CHIP.roxy, `CHIP.puxa, !`CHIP.nyze_n, `CHIP.paho, `CHIP.roga,
	              `CHIP.rubu, `CHIP.ryku})
	`TRACE(sprite_fetch, {`CHIP.toxe, `CHIP.tuly, `CHIP.tese, `CHIP.sobu, !`CHIP.suda_n, `CHIP.taka,
	                      `CHIP.tobu, `CHIP.vonu, `CHIP.seba, `CHIP.tyfo})
	`TRACE(scy, {`CHIP.ff42_d7, `CHIP.ff42_d6, `CHIP.ff42_d5, `CHIP.ff42_d4, `CHIP.ff42_d3, `CHIP.ff42_d2,
	             `CHIP.ff42_d1, `CHIP.ff42_d0})
	`TRACE(scx, {`CHIP.ff43_d7, `CHIP.ff43_d6, `CHIP.ff43_d5, `CHIP.ff43_d4, `CHIP.ff43_d3, `CHIP.ff43_d2,
	             `CHIP.ff43_d1, `CHIP.ff43_d0})
	`TRACE(wy, {`CHIP.ff4a_d7, `CHIP.ff4a_d6, `CHIP.ff4a_d5, `CHIP.ff4a_d4, `CHIP.ff4a_d3, `CHIP.ff4a_d2,
	            `CHIP.ff4a_d1, `CHIP.ff4a_d0})
	`TRACE(wx, {`CHIP.ff4b_d7, `CHIP.ff4b_d6, `CHIP.ff4b_d5, `CHIP.ff4b_d4, `CHIP.ff4b_d3, `CHIP.ff4b_d2,
	            `CHIP.ff4b_d1, `CHIP.ff4b_d0})
	`TRACE(win_y, {`CHIP.teke, `CHIP.tate, `CHIP.tozo, `CHIP.taxa, `CHIP.tufu, `CHIP.vymu, `CHIP.vujo,
	               `CHIP.vyno})
	`TRACE(win_x, {`CHIP.xolo, `CHIP.wyko, `CHIP.wobo, `CHIP.wody, `CHIP.wyka})
	`TRACE(bg_tile, {`CHIP.bg_tile7, `CHIP.bg_tile6, `CHIP.bg_tile5, `CHIP.bg_tile4, `CHIP.bg_tile3,
	                 `CHIP.bg_tile2, `CHIP.bg_tile1, `CHIP.bg_tile0})
	`TRACE(bg_low, {`CHIP.nefo, `CHIP.nasa, `CHIP.myjy, `CHIP.megu, `CHIP.luzo, `CHIP.muku, `CHIP.nudu,
	                `CHIP.legu})
	`TRACE(bg_shift_a, {`CHIP.bg_px_a, `CHIP.neda, `CHIP.modu, `CHIP.nepo, `CHIP.macu, `CHIP.moju, `CHIP.nozo,
	                    `CHIP.myde})
	`TRACE(bg_shift_b, {`CHIP.bg_px_b, `CHIP.ralu, `CHIP.setu, `CHIP.sobo, `CHIP.rysa, `CHIP.sady, `CHIP.taca,
	                    `CHIP.tomy})
	`TRACE(bgp, {`CHIP.bgp_d7, `CHIP.bgp_d6, `CHIP.bgp_d5, `CHIP.bgp_d4, `CHIP.bgp_d3, `CHIP.bgp_d2,
	             `CHIP.bgp_d1, `CHIP.bgp_d0})
	`TRACE(obp0, {`CHIP.obp0_d7, `CHIP.obp0_d6, `CHIP.obp0_d5, `CHIP.obp0_d4, `CHIP.obp0_d3, `CHIP.obp0_d2,
	              `CHIP.obp0_d1, `CHIP.obp0_d0})
	`TRACE(obp1, {`CHIP.obp1_d7, `CHIP.obp1_d6, `CHIP.obp1_d5, `CHIP.obp1_d4, `CHIP.obp1_d3, `CHIP.obp1_d2,
	              `CHIP.obp1_d1, `CHIP.obp1_d0})
	`TRACE(sp_low_a, {`CHIP.rydu, `CHIP.rama, `CHIP.suto, `CHIP.sele, `CHIP.ramu, `CHIP.mytu, `CHIP.roka,
	                  `CHIP.pefo})
	`TRACE(sp_low_b, {`CHIP.sega, `CHIP.semo, `CHIP.suny, `CHIP.saja, `CHIP.pudu, `CHIP.mofo, `CHIP.peba,
	                  `CHIP.rewo})
	`TRACE(sp_shift_a, {`CHIP.sprite_px_a7, `CHIP.sprite_px_a6, `CHIP.sprite_px_a5, `CHIP.sprite_px_a4,
	                    `CHIP.sprite_px_a3, `CHIP.sprite_px_a2, `CHIP.sprite_px_a1, `CHIP.sprite_px_a0})
	`TRACE(sp_shift_b, {`CHIP.sprite_px_b7, `CHIP.sprite_px_b6, `CHIP.sprite_px_b5, `CHIP.sprite_px_b4,
	                    `CHIP.sprite_px_b3, `CHIP.sprite_px_b2, `CHIP.sprite_px_b1, `CHIP.sprite_px_b0})
	`TRACE(sp_shift_pal, {`CHIP.sprite_px_palette, `CHIP.moda, `CHIP.nuke, `CHIP.palu, `CHIP.somy, `CHIP.rosa,
	                      `CHIP.sata, `CHIP.rugo})
	`TRACE(sp_shift_prio, {`CHIP.sprite_px_priority, `CHIP.vumo, `CHIP.woda, `CHIP.xete, `CHIP.wyfu,
	                       `CHIP.vosa, `CHIP.wuru, `CHIP.vezo})
	`TRACE(oam_addr, {`CHIP.oam_a7, `CHIP.oam_a6, `CHIP.oam_a5, `CHIP.oam_a4, `CHIP.oam_a3, `CHIP.oam_a2,
	                  `CHIP.oam_a1, !`CHIP.oam_a0_n_tri})
	`TRACE(oam_a_bus, {!`CHIP.oam_a_d7_n, !`CHIP.oam_a_d6_n, !`CHIP.oam_a_d5_n, !`CHIP.oam_a_d4_n,
	                   !`CHIP.oam_a_d3_n, !`CHIP.oam_a_d2_n, !`CHIP.oam_a_d1_n, !`CHIP.oam_a_d0_n})
	`TRACE(oam_b_bus, {!`CHIP.oam_b_d7_n, !`CHIP.oam_b_d6_n, !`CHIP.oam_b_d5_n, !`CHIP.oam_b_d4_n,
	                   !`CHIP.oam_b_d3_n, !`CHIP.oam_b_d2_n, !`CHIP.oam_b_d1_n, !`CHIP.oam_b_d0_n})
	`TRACE(oam_ctl, {`CHIP.oam_oe, `CHIP.oam_a_wr2, `CHIP.oam_b_wr2, !`CHIP.oam_bl_precharge_n,
	                 !`CHIP.oam_wldrv_precharge_n, `CHIP.oam_data_latch, !`CHIP.oam_addr_cpu_n,
	                 !`CHIP.oam_addr_dma_n, !`CHIP.oam_addr_parse_n, !`CHIP.oam_addr_render_n})
	`TRACE(oam_latch_b, ~{`CHIP.zeca, `CHIP.yses, `CHIP.xafu, `CHIP.zaxe, `CHIP.\wone , `CHIP.zuca, `CHIP.yceb,
	                     `CHIP.ydyv})
	`TRACE(oam_latch_y, {`CHIP.yzab_n, `CHIP.xote_n, `CHIP.wyso_n, `CHIP.ybog_n, `CHIP.xyju_n, `CHIP.yjex_n,
	                     `CHIP.xegu_n, `CHIP.xuso_n})
	`TRACE(ma, {!`CHIP.ma12_n, !`CHIP.ma11_n, !`CHIP.ma10_n, !`CHIP.ma9_n, !`CHIP.ma8_n, !`CHIP.ma7_n,
	            !`CHIP.ma6_n, !`CHIP.ma5_n, !`CHIP.ma4_n, !`CHIP.ma3_n, !`CHIP.ma2_n, !`CHIP.ma1_n,
	            !`CHIP.ma0_n})
	`TRACE(oam_latch_a, ~{`CHIP.eced, `CHIP.zuve, `CHIP.cyra, `CHIP.wyno, `CHIP.yvel, `CHIP.ysex, `CHIP.yrum,
	                      `CHIP.xyky})
	`TRACE(oam_latch_x, {`CHIP.sprite_priority, `CHIP.sprite_y_flip, `CHIP.sprite_x_flip, `CHIP.sprite_palette,
	                     `CHIP.zezy_n, `CHIP.zyve_n, `CHIP.zyty_n, `CHIP.ylor_n})
	`TRACE(md_out, !`CHIP.rahu)
	`TRACE(ma_pins, `CHIP.ma_pad)
	`TRACE(md_pins, `CHIP.md_pad)
	`TRACE(mcs_n, `CHIP.mcs_n_pad)
	`TRACE(mrd_n, `CHIP.mrd_n_pad)
	`TRACE(mwr_n, `CHIP.mwr_n_pad)
	`TRACE(md, {`CHIP.md7, `CHIP.md6, `CHIP.md5, `CHIP.md4, `CHIP.md3, `CHIP.md2, `CHIP.md1, `CHIP.md0})

	// The sprite store: each slot's X, line and index in OAM
	`TRACE(sprite_x0, ~{`CHIP.xako, `CHIP.wote, `CHIP.xuny, `CHIP.welo, `CHIP.zulu, `CHIP.zola, `CHIP.ylah, `CHIP.xepe})
	`TRACE(sprite_x1, ~{`CHIP.cusy, `CHIP.dyfu, `CHIP.ceso, `CHIP.dake, `CHIP.dazo, `CHIP.desu, `CHIP.duko, `CHIP.dany})
	`TRACE(sprite_x2, ~{`CHIP.depy, `CHIP.enor, `CHIP.ejuf, `CHIP.duhy, `CHIP.goxu, `CHIP.fuby, `CHIP.fyty, `CHIP.foka})
	`TRACE(sprite_x3, ~{`CHIP.weco, `CHIP.wyna, `CHIP.wuha, `CHIP.xomy, `CHIP.xeka, `CHIP.xabe, `CHIP.xyba, `CHIP.xoly})
	`TRACE(sprite_x4, ~{`CHIP.xepa, `CHIP.wyde, `CHIP.zala, `CHIP.ybed, `CHIP.xury, `CHIP.zyjo, `CHIP.ygaj, `CHIP.wedu})
	`TRACE(sprite_x5, ~{`CHIP.cuvy, `CHIP.dury, `CHIP.dyby, `CHIP.cywe, `CHIP.fesy, `CHIP.fozy, `CHIP.faxa, `CHIP.fusa})
	`TRACE(sprite_x6, ~{`CHIP.zesa, `CHIP.zoly, `CHIP.zogo, `CHIP.zecu, `CHIP.yvag, `CHIP.ymem, `CHIP.yrac, `CHIP.ycol})
	`TRACE(sprite_x7, ~{`CHIP.fede, `CHIP.exuk, `CHIP.faxe, `CHIP.fazu, `CHIP.ehyn, `CHIP.erol, `CHIP.epum, `CHIP.eraz})
	`TRACE(sprite_x8, ~{`CHIP.fyca, `CHIP.ebow, `CHIP.enad, `CHIP.ezuf, `CHIP.gaza, `CHIP.gady, `CHIP.gypu, `CHIP.gavy})
	`TRACE(sprite_x9, ~{`CHIP.yzof, `CHIP.ynep, `CHIP.yrop, `CHIP.ypod, `CHIP.xexa, `CHIP.xuzo, `CHIP.xere, `CHIP.xuvy})
	`TRACE(sprite_line0, ~{`CHIP.gyho_n, `CHIP.bozu_n, `CHIP.cufo_n, `CHIP.fyhy_n})
	`TRACE(sprite_line1, ~{`CHIP.abug_n, `CHIP.abop_n, `CHIP.arof_n, `CHIP.ames_n})
	`TRACE(sprite_line2, ~{`CHIP.ykuk_n, `CHIP.xazy_n, `CHIP.xosy_n, `CHIP.ylov_n})
	`TRACE(sprite_line3, ~{`CHIP.zury_n, `CHIP.zene_n, `CHIP.zylu_n, `CHIP.zuro_n})
	`TRACE(sprite_line4, ~{`CHIP.cumu_n, `CHIP.cono_n, `CHIP.caju_n, `CHIP.capo_n})
	`TRACE(sprite_line5, ~{`CHIP.aned_n, `CHIP.abux_n, `CHIP.abeg_n, `CHIP.acep_n})
	`TRACE(sprite_line6, ~{`CHIP.zube_n, `CHIP.zexo_n, `CHIP.zafu_n, `CHIP.zumy_n})
	`TRACE(sprite_line7, ~{`CHIP.xygo_n, `CHIP.xaku_n, `CHIP.ygum_n, `CHIP.xyna_n})
	`TRACE(sprite_line8, ~{`CHIP.afym_n, `CHIP.afut_n, `CHIP.afyx_n, `CHIP.azap_n})
	`TRACE(sprite_line9, ~{`CHIP.dewu_n, `CHIP.dysy_n, `CHIP.fofo_n, `CHIP.cana_n})
	`TRACE(sprite_index0, ~{`CHIP.ygus_n, `CHIP.ysok_n, `CHIP.yzep_n, `CHIP.wyte_n, `CHIP.zony_n, `CHIP.ywak_n})
	`TRACE(sprite_index1, ~{`CHIP.cajy_n, `CHIP.cuza_n, `CHIP.coma_n, `CHIP.cufa_n, `CHIP.cebo_n, `CHIP.cadu_n})
	`TRACE(sprite_index2, ~{`CHIP.boxa_n, `CHIP.buna_n, `CHIP.bulu_n, `CHIP.beca_n, `CHIP.byhu_n, `CHIP.buhe_n})
	`TRACE(sprite_index3, ~{`CHIP.dafu_n, `CHIP.deba_n, `CHIP.duha_n, `CHIP.duny_n, `CHIP.dese_n, `CHIP.devy_n})
	`TRACE(sprite_index4, ~{`CHIP.xynu_n, `CHIP.xege_n, `CHIP.xabo_n, `CHIP.wanu_n, `CHIP.xefe_n, `CHIP.xave_n})
	`TRACE(sprite_index5, ~{`CHIP.ekap_n, `CHIP.etav_n, `CHIP.ebex_n, `CHIP.goru_n, `CHIP.etym_n, `CHIP.ekop_n})
	`TRACE(sprite_index6, ~{`CHIP.gecu_n, `CHIP.foxy_n, `CHIP.gohu_n, `CHIP.fogo_n, `CHIP.gacy_n, `CHIP.gabo_n})
	`TRACE(sprite_index7, ~{`CHIP.fuzo_n, `CHIP.gesy_n, `CHIP.fysu_n, `CHIP.fefa_n, `CHIP.gyno_n, `CHIP.gule_n})
	`TRACE(sprite_index8, ~{`CHIP.axuv_n, `CHIP.bada_n, `CHIP.apev_n, `CHIP.bado_n, `CHIP.bexy_n, `CHIP.byhe_n})
	`TRACE(sprite_index9, ~{`CHIP.xufo_n, `CHIP.xute_n, `CHIP.xotu_n, `CHIP.xyfe_n, `CHIP.yzor_n, `CHIP.yber_n})
endmodule

`default_nettype wire
