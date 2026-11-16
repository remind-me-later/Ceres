; SPDX-License-Identifier: MIT
;
; A stimulus for comparing ceres with the readable model (see ../README.md):
; with WX = A6, WY = 0 and a sprite at X = A7 on line 0 (as in gambatte's
; m2int_wxA6_spxA7 and enable_wxA6_2x_spxA7 ROMs), it reads STAT every two
; M-cycles across the end of line 0's mode 3, twice: the second time a frame
; later, less an M-cycle. The window is turned on in VBlank before each pass,
; and the second pass turns it off again in line 1 of the first's frame: in
; the first pass it was off in the visible lines of the frame before, in the
; second only from line 1 on.
; Each pass starts when HALT ends at line 153's LY=LYC interrupt, a fixed time
; into the line, so that between them the passes read every M-cycle from
; there on. The background is colour 1, the window colour 3 (its map at
; 9C00), the sprite colour 2. The bus traces, the frames and the pixel
; timelines of compare-ceres.sh show what each read gives, and what each line
; draws:
;
;   BOOT=tiny PIXELS=1 CUTS=1 ./compare-ceres.sh hdl/build/window_line0_sprite.gb 0.12
;
; On the DMG, the window starts at the end of line 0, after its last pixel
; (WX matches at the last dot), and the fetch of the sprite waits for the
; window's first tiles: mode 3 lasts three M-cycles more than on line 1,
; where the window is already on. The next line starts in the window if WX
; was 166 at the end of the line before, whether or not the window was on
; then: in the second pass, line 0 is the window's too.
; @fact window-wx-166
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
	; Wait for VBlank, to turn the LCD off.
.vblank:
	ldh a, [$FF44]
	cp 145
	jr nz, .vblank
	xor a
	ldh [$FF40], a            ; the LCD off, to write VRAM

	ld hl, $8000
	ld b, 8
.tile0:
	ld a, $FF                 ; tile 0, the background's: colour 1
	ld [hl+], a
	xor a
	ld [hl+], a
	dec b
	jr nz, .tile0
	ld b, 16
.tile1:
	ld a, $FF                 ; tile 1, the window's: colour 3
	ld [hl+], a
	dec b
	jr nz, .tile1
	ld b, 8
.tile2:
	xor a                     ; tile 2, the sprite's: colour 2
	ld [hl+], a
	ld a, $FF
	ld [hl+], a
	dec b
	jr nz, .tile2
	ld hl, $9C00              ; the window's map
	ld bc, $0400
.map:
	ld a, 1
	ld [hl+], a
	dec bc
	ld a, b
	or c
	jr nz, .map
	ld a, $E4
	ldh [$FF47], a            ; BGP
	ldh [$FF48], a            ; OBP0
	ld a, $D3
	ldh [$FF40], a            ; the LCD on, the window off

	; Wait for VBlank, to write OAM.
.vblank1:
	ldh a, [$FF44]
	cp 145
	jr nz, .vblank1

	ld hl, $FE00
	ld a, 16
	ld [hl+], a               ; sprite 0: Y on line 0
	ld a, $A7
	ld [hl+], a               ; X: its last pixel at the line's end
	ld a, 2
	ld [hl+], a               ; tile 2
	xor a
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
	ld a, $D3
	ldh [$FF40], a            ; the window off for the visible lines
.vblank2:
	ldh a, [$FF44]
	cp 145
	jr nz, .vblank2
	ld a, $F3
	ldh [$FF40], a            ; LCD, window (map 9C00), sprites and background on

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
