; SPDX-License-Identifier: MIT
;
; The window is turned off (LCDC bit 5) while the background fetcher is in
; the middle of a tile that began in the window.
;
; The DMG's "in window" falls with the bit at once, and the fetcher picks
; from it when it reads, not when the fetch began: the tile's number comes
; from the background's map if the bit falls before the read, and its data
; bytes from the background's row. (Ceres decided both when the fetch began,
; until commit 9429e718.)
;
; The window shows tile 1 (rows alternate black and light gray) from the map
; at $9C00; the background shows tile 2 (dark gray) from $9800; SCY is 1, so
; the background's row has the opposite parity to the window's. Every line
; writes WX = LY mod 8 at M-cycle 19, turns the window off at M-cycle 22 and
; on at 24, so the off moment meets a different phase of the fetch in each of
; eight consecutive lines, from the fetch of the first tile (WX 0 to 7 starts
; the window before the line's first pixel).
;
; Expected picture (the netlist's, which the readable model follows; colours
; 0 white, 1 light gray, 2 dark gray, 3 black): the first 16 pixels of a line,
; by the line's number mod 8. It repeats every eight lines. 2 is the
; background map's tile; 1 and 3 are stripes of the window map's tile 1.
;
;   LY mod 8  pixels
;   0         3111111112222222
;   1         1133333333222222
;   2         3332222222222222
;   3         1111222222222222
;   4         1111122222222222   (ceres: 3333322222222222)
;   5         3333332222222222
;   6         1111111222222222
;   7         3333333322222222
;
; Ceres before commit 9429e718 chose the map and the row when the fetch began,
; and was off by hundreds of pixels (900 of 23040 in the picture); now only
; the line 4 differs (90 pixels), which looks like one dot in the order of
; the write and the data read (not diagnosed). The Mealybug screenshot of the related m3_lcdc_win_en_change_multiple_wx
; agrees with the netlist. No screenshot of this ROM exists yet.
;
;   make win_off_during_tile_fetch.gb

INCLUDE "common.inc"

DEF LCDC_WIN_ON  EQU LCDC_ON | LCDC_WMAP | LCDC_WIN | LCDC_TILE | LCDC_BG
DEF LCDC_WIN_OFF EQU LCDC_ON | LCDC_WMAP | LCDC_TILE | LCDC_BG

; M-cycles between turning the LCD on and the top of the line loop, so that
; the loop's cycle 0 is the start of a line (tuned with `trace --timing`).
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
	ld a, 2
	fill_bc
	ld hl, MAP1
	ld bc, $0400
	ld a, 1
	fill_bc

	ld a, 1
	ldh [rSCY], a
	xor a
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
	m_nop 6
	m_read rLY
	and a, 7
	DEF cyc += 2
	m_write_a_at 19, rWX
	m_write_hl_at 22, b
	m_write_hl_at 24, c
	m_line_end .line

tiles:
	tile_solid 0
	tile_rows 3, 1, 3, 1, 3, 1, 3, 1
	tile_solid 2
tiles_end:
