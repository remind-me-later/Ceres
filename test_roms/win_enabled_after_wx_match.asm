; SPDX-License-Identifier: MIT
;
; The window is enabled (LCDC bit 5) a moment after the pixel counter has
; reached WX.
;
; The window is off from M-cycle 15 of every line and on again at M-cycle 24
; (the pixel that is being drawn then is about the tenth). WX is 12 plus the
; line's number mod 8. The window is black, the background white.
;
; What to look at, line by line (the netlist, which the model follows, and
; ceres since commit 9429e718):
;
;   WX 12 to 15: the match comes before the enable: no window in the line.
;   WX 16:       the match meets the enable: the window starts one pixel late,
;                at x = 10, and keeps the pixel before it (x = 9, white).
;   WX 17 to 19: the window starts at x = WX - 7.
;
; SameBoy, and ceres before that commit, step the LCD's column back in the WX
; 16 line, so that the window's first pixel overwrites x = 9 (black from
; x = 9). The Mealybug screenshot of m3_lcdc_win_en_change_multiple_wx, from the
; real DMG, agrees with the netlist.
;
;   make win_enabled_after_wx_match.gb

INCLUDE "common.inc"

DEF LCDC_WIN_ON  EQU LCDC_ON | LCDC_WMAP | LCDC_WIN | LCDC_TILE | LCDC_BG
DEF LCDC_WIN_OFF EQU LCDC_ON | LCDC_WMAP | LCDC_TILE | LCDC_BG

DEF LCD_SYNC EQU 0

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	wait_vblank
	xor a
	ldh [rLCDC], a

	ld hl, tiles
	ld de, VRAM_TILES
	ld bc, tiles_end - tiles
	copy_bc
	ld hl, MAP0
	ld bc, $0400
	xor a
	fill_bc
	ld hl, MAP1
	ld bc, $0400
	ld a, 1
	fill_bc

	xor a
	ldh [rSCY], a
	ldh [rSCX], a
	ldh [rWY], a
	ld a, $E4
	ldh [rBGP], a

	ld hl, rLCDC
	ld b, LCDC_WIN_OFF
	ld c, LCDC_WIN_ON
	ld a, c
	ldh [rLCDC], a
	REPT LCD_SYNC
		nop
	ENDR

DEF cyc = 0
.line:
	m_nop 3
	m_read rLY
	and a, 7
	add a, 12
	DEF cyc += 4
	m_write_a_at 12, rWX
	m_write_hl_at 15, b
	m_write_hl_at 24, c
	m_line_end .line

tiles:
	tile_solid 0
	tile_solid 3
tiles_end:
