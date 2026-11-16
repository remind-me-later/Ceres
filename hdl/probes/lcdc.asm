; SPDX-License-Identifier: MIT
;
; LCDC's writes and reads: the CPU writes every value to FF40 in turn (a
; step of $45 that visits all 256), with a pause that varies, so that the
; LCD goes off and on and the sprites' height changes at many points of the
; line and of the scan, and reads each back. For trace-blocks.sh, whose
; traces hold the PPU's reset, LCDC bit 2 and the byte each read takes:
;
;   ./trace-blocks.sh hdl/build/lcdc.gb 0.1
;
;   rgbasm -o lcdc.o lcdc.asm && rgblink -o lcdc.gb lcdc.o && rgbfix -v -p 0 lcdc.gb

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	ld sp, $FFFE
	ldh a, [$FF40]              ; as the boot left it
	ld c, $91
.loop:
	ld a, c
	ldh [$FF40], a
	ldh a, [$FF40]
	; A pause of 20 to 35 loops, so that the writes fall all over the line.
	ld a, c
	and 15
	add 20
	ld b, a
.pause:
	dec b
	jr nz, .pause
	ld a, c
	add $45
	ld c, a
	jr .loop
