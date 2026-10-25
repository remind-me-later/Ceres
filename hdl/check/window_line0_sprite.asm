; SPDX-License-Identifier: MIT
;
; A stimulus for comparing ceres with the readable model (see ../README.md):
; with WX = A6, WY = 0 and a sprite at X = A7 on line 0 (as in gambatte's
; m2int_wxA6_spxA7 and enable_wxA6_2x_spxA7 ROMs), it reads STAT every two
; M-cycles across the end of line 0's mode 3, twice: the second time a frame
; later, less an M-cycle. The window is off in the visible lines before each
; pass and turned on in VBlank: with the window on in the frame before, line
; 0 is the same on both sides. Each pass starts when HALT ends at line 153's
; LY=LYC interrupt, a fixed time into the line, so that between them the
; passes read every M-cycle from there on. The bus traces of compare-ceres.sh
; show what each read gives:
;
;   BOOT=tiny NOFRAMES=1 CUTS=1 ./compare-ceres.sh hdl/build/window_line0_sprite.gb 0.08
;
; The model starts the window at the end of line 0 (WX matches at the pixel
; before the sprite's), which stalls mode 3: its STAT shows mode 3 for four
; M-cycles more than ceres's (results.md).
;
; It reports no result, and avoids the byte 40 as an opcode (gameboy_tb's
; TEST takes LD B,B for a Mooneye result).
;
;   rgbasm -o window_line0_sprite.o window_line0_sprite.asm && rgblink -o window_line0_sprite.gb window_line0_sprite.o && rgbfix -v -p 0 window_line0_sprite.gb

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	; Wait for VBlank, to write OAM.
.vblank:
	ldh a, [$FF44]
	cp 145
	jr nz, .vblank

	ld hl, $FE00
	ld a, 16
	ld [hl+], a               ; sprite 0: Y on line 0
	ld a, $A7
	ld [hl+], a               ; X: its last pixel at the line's end
	xor a
	ld [hl+], a               ; tile 0
	ld [hl+], a               ; attributes
	ld a, 160
	ld b, 39 * 4
.hide:
	ld [hl+], a               ; the other sprites below the screen
	dec b
	jr nz, .hide

	xor a
	ldh [$FF4A], a            ; WY
	ld a, $A6
	ldh [$FF4B], a            ; WX

	ld a, 153
	ldh [$FF45], a            ; LYC
	ld a, $40
	ldh [$FF41], a            ; the STAT interrupt on LY=LYC
	ld a, $02
	ldh [$FFFF], a            ; IE: STAT only
	ld c, LOW($FF41)          ; STAT, for LDH A,(C)
	ld b, 0                   ; the pass: 0, then 1

.pass:
	ld a, $93
	ldh [$FF40], a            ; the window off for the visible lines
.vblank2:
	ldh a, [$FF44]
	cp 145
	jr nz, .vblank2
	ld a, $B3
	ldh [$FF40], a            ; LCD, window, sprites and background on

	; Wait in HALT for line 153's LY=LYC (IME is off: no dispatch). IF is
	; cleared first, so that HALT waits.
	xor a
	ldh [$FF0F], a
	halt
	nop

	; The second pass reads an M-cycle sooner: JR takes 3 M-cycles when it
	; jumps, 2 when it does not.
	ld a, b
	rra
	jr nc, .wait
.wait:
	; To line 0's mode 3 (line 153 lasts 114 M-cycles).
	REPT 160
	nop
	ENDR
	; 120 M-cycles of reads, past the end of line 0's mode 3.
	REPT 60
	ldh a, [c]
	ENDR

	inc b
	ld a, b
	cp 2
	jp nz, .pass

.done:
	jr .done
