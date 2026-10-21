; SPDX-License-Identifier: MIT
;
; Which byte of a sprite's tile row is which bit plane of its pixels.
;
; A pixel's colour number is 2 * high bit + low bit, the low bit from the row's
; first byte (even address) and the high bit from the second. Tile 1 has only
; the low plane set (every pixel of colour 1), tile 2 only the high plane
; (colour 2), tile 3 both, in different pixels (F0 / CC), and the palettes
; make the colour numbers visible: OBP0 is the identity, OBP1 reverses it.
;
; Expected picture (colours 0 white, 1 light gray, 2 dark gray, 3 black), the
; same in every line of a sprite, which is eight lines from line 8:
;
;   sprite 1 (x  8 to 15, OBP0): 11111111
;   sprite 2 (x 24 to 31, OBP0): 22222222
;   sprite 3 (x 40 to 47, OBP0): 33112200   (3 3 1 1 2 2 0 0)
;   sprite 4 (x 56 to 63, OBP1): 00221100   (the same tile through OBP1)
;
; The readable model of the PPU had the planes swapped until commit 43ae47da
; (sprite 1 came out as 22222222): the netlist's palette cells pick 2 * A + B,
; with A latched from the tile's second byte. hdl/check/sprites.asm is the
; same stimulus, for the checks against dmg-sim.
;
;   make sprite_bit_planes.gb

INCLUDE "common.inc"

DEF LCDC_SPRITES EQU LCDC_ON | LCDC_TILE | LCDC_OBJ | LCDC_BG

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

	; OAM: the four sprites, then the rest off the screen (Y 0).
	ld hl, $FE00
	ld de, sprites
	ld b, sprites_end - sprites
.oam:
	ld a, [de]
	inc de
	ld [hl+], a
	dec b
	jr nz, .oam
.oam_rest:
	xor a
	ld [hl+], a
	ld a, l
	cp $A0
	jr nz, .oam_rest

	xor a
	ldh [rSCY], a
	ldh [rSCX], a
	ld a, $E4
	ldh [rBGP], a
	ldh [rOBP0], a
	ld a, $1B
	ldh [rOBP1], a

	ld a, LCDC_SPRITES
	ldh [rLCDC], a
.stop:
	jr .stop

; Y + 16, X + 8, tile, attributes (bit 4: OBP1)
sprites:
	db 24, 16, 1, $00
	db 24, 32, 2, $00
	db 24, 48, 3, $00
	db 24, 64, 3, $10
sprites_end:

tiles:
	tile_solid 0
	REPT 8
		db $FF, $00      ; tile 1: the low plane only
	ENDR
	REPT 8
		db $00, $FF      ; tile 2: the high plane only
	ENDR
	REPT 8
		db $F0, $CC      ; tile 3: both, in different pixels
	ENDR
tiles_end:
