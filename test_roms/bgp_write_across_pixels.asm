; SPDX-License-Identifier: MIT
;
; BGP is written in the middle of a line, with the pixel being drawn at a
; different dot of the pixel clock in each of eight lines.
;
; Every pixel is colour 1; BGP maps it to light gray ($04) and, from the write
; at M-cycle 40, to dark gray ($08). SCX is the line's number mod 8: it delays
; the start of the line's pixels by that many dots, which moves the pixel at
; the write by one for each line, through all eight positions of a dot.
;
; What to look at: each line is light gray up to a column and dark gray from
; it, and in the real DMG (by the screenshot of Mealybug's m3_bgp_change,
; which has this black pixel at every write of BGP; no screenshot of this ROM
; exists yet) the last light pixel is black: a black pixel just before the
; first dark gray, in all eight lines. Ceres draws it. The readable model, and
; dmg-sim, do not: their register does show FF for about 39 ns while the
; write's data is on its way, but no pixel is taken in that moment. A test
; that expects the black pixel fails on the model.
;
;   make bgp_write_across_pixels.gb

INCLUDE "common.inc"

DEF LCDC_BG_ONLY EQU LCDC_ON | LCDC_TILE | LCDC_BG

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
	ld a, 1
	fill_bc

	xor a
	ldh [rSCY], a
	ldh [rSCX], a
	ldh [rWY], a
	ld b, $08
	ld c, $04
	ld a, c
	ldh [rBGP], a

	ld a, LCDC_BG_ONLY
	ldh [rLCDC], a
	REPT LCD_SYNC
		nop
	ENDR

DEF cyc = 0
.line:
	m_nop 3
	m_read rLY
	and a, 7
	DEF cyc += 2
	m_write_a_at 12, rSCX
	m_write_r_at 40, rBGP, b
	m_write_r_at 100, rBGP, c
	m_line_end .line

tiles:
	tile_solid 0
	tile_solid 1
tiles_end:
