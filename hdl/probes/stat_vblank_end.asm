; SPDX-License-Identifier: MIT
;
; A stimulus for comparing ceres with the readable model (see ../README.md):
; it reads STAT every two M-cycles across the end of VBlank (the end of line
; 153), twice: the second time a frame later, less an M-cycle. Each pass starts
; when HALT ends at line 152's LY=LYC interrupt, a fixed time into the line,
; so that between them the passes read every M-cycle from there on. The bus
; traces of compare-ceres.sh show what each read gives:
;
;   NOFRAMES=1 ./compare-ceres.sh hdl/build/stat_vblank_end.gb 0.06
;
; Both sides read mode 1, then mode 0 for one M-cycle, then line 0's mode 2
; (results.md). It reports no result, and avoids the byte 40 as an opcode
; (gameboy_tb's TEST takes LD B,B for a Mooneye result).
;
;   rgbasm -o stat_vblank_end.o stat_vblank_end.asm && rgblink -o stat_vblank_end.gb stat_vblank_end.o && rgbfix -v -p 0 stat_vblank_end.gb

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	ld a, 152
	ldh [$FF45], a            ; LYC
	ld a, $40
	ldh [$FF41], a            ; the STAT interrupt on LY=LYC
	ld a, $02
	ldh [$FFFF], a            ; IE: STAT only
	ld c, LOW($FF41)          ; STAT, for LDH A,(C)
	ld b, 0                   ; the pass: 0, then 1

.pass:
	; Wait in HALT for line 152's LY=LYC (IME is off: no dispatch). IF is
	; cleared first, so that HALT waits.
	xor a
	ldh [$FF0F], a
	halt
	nop

	; The second pass reads an M-cycle sooner: JR takes 3 M-cycles when it
	; jumps, 2 when it does not.
	ld a, b
	rra
	jr nc, .read
.read:
	; 400 M-cycles of reads, past the end of line 153 (228 M-cycles after
	; line 152 begins).
	REPT 200
	ldh a, [c]
	ENDR

	inc b
	ld a, b
	cp 2
	jp nz, .pass

.done:
	jr .done
