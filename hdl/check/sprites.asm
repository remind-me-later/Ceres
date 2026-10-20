; SPDX-License-Identifier: MIT
;
; A stimulus for the PPU's check (see ../README.md): sprites whose tiles
; tell the two bit planes apart. Tile 1 has only the low plane set (pixels
; of colour 1), tile 2 only the high plane (colour 2), tile 3 both in
; different bits, and the palettes are the identity, so that the picture
; shows which byte of a tile each plane comes from. It reports no result.
;
;   rgbasm -o sprites.o sprites.asm && rgblink -o sprites.gb sprites.o && rgbfix -v -p 0 sprites.gb

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	xor a
	ldh [$FF40], a            ; the LCD off: VRAM and OAM are free

	; Tile 0 and the background map cleared: the boot ROM that started us
	; may not have.
	ld hl, $8000
	ld bc, $0010
	call clear
	ld hl, $9800
	ld bc, $0400
	call clear

	; Tile 1: low plane FF, high plane 00. Tile 2: the other way round.
	; Tile 3: low plane F0, high plane CC.
	ld hl, $8010
	ld b, 8
.tile1:
	ld a, $FF
	ld [hl+], a
	xor a
	ld [hl+], a
	dec b
	jr nz, .tile1
	ld b, 8
.tile2:
	xor a
	ld [hl+], a
	ld a, $FF
	ld [hl+], a
	dec b
	jr nz, .tile2
	ld b, 8
.tile3:
	ld a, $F0
	ld [hl+], a
	ld a, $CC
	ld [hl+], a
	dec b
	jr nz, .tile3

	; Three sprites on line 8, and one over another with the other palette.
	ld hl, $FE00
	ld de, sprites
	ld b, 16
.oam:
	ld a, [de]
	inc de
	ld [hl+], a
	dec b
	jr nz, .oam

	ld a, $E4
	ldh [$FF47], a
	ldh [$FF48], a
	ld a, $1B
	ldh [$FF49], a            ; the other palette: inverted
	ld a, $93
	ldh [$FF40], a            ; LCD, sprites and background on, tiles at 8000
.wait:
	jr .wait

; Zeroes BC bytes from HL.
clear:
	xor a
	ld [hl+], a
	dec bc
	ld a, b
	or c
	jr nz, clear
	ret

; y + 16, x + 8, tile, attributes
sprites:
	db 24, 16, 1, $00
	db 24, 32, 2, $00
	db 24, 48, 3, $00
	db 24, 64, 3, $10       ; OBP1
