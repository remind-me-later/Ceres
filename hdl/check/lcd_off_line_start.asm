; SPDX-License-Identifier: MIT
;
; A stimulus for comparing ceres with the readable model (see ../README.md):
; it turns the LCD off around the start of line 69 with STAT's mode 2
; interrupt enabled, and reads IF right after, in 16 passes, each an M-cycle
; later than the one before. Each pass starts when HALT ends at line 68's
; LY=LYC interrupt, a fixed time into the line, and turns the LCD on again at
; its end, so that the next pass waits for line 68 of a new frame. IME is
; off: nothing is dispatched. The bus traces of compare-ceres.sh show what
; each read of IF gives:
;
;   NOFRAMES=1 CUTS=1 ./compare-ceres.sh hdl/build/lcd_off_line_start.gb 0.15
;
; It reports no result, and avoids the byte 40 as an opcode (gameboy_tb's
; TEST takes LD B,B for a Mooneye result).
;
;   rgbasm -o lcd_off_line_start.o lcd_off_line_start.asm && rgblink -o lcd_off_line_start.gb lcd_off_line_start.o && rgbfix -v -p 0 lcd_off_line_start.gb

DEF LINE  EQU 68                  ; the line before the one whose start is swept
DEF DELAY EQU 90                  ; the first pass's NOPs; the last's is 105

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	ld a, $02
	ldh [$FFFF], a            ; IE: STAT only
	ld a, LINE
	ldh [$FF45], a            ; LYC

	FOR K, 16
	; Wait in HALT for line 68's LY=LYC. STAT's write sets IF itself on the
	; DMG (the write glitch), so IF is cleared after it.
	ld a, $40
	ldh [$FF41], a            ; the STAT interrupt on LY=LYC alone
	xor a
	ldh [$FF0F], a
	halt
	nop

	; Now the mode 2 source alone, and IF cleared (the write glitch again:
	; LY=LYC holds).
	ld a, $20
	ldh [$FF41], a
	xor a
	ldh [$FF0F], a

	REPT DELAY + K
	nop
	ENDR
	ld a, $11
	ldh [$FF40], a            ; the LCD off
	ldh a, [$FF0F]            ; IF, on the bus
	ld a, $91
	ldh [$FF40], a            ; the LCD on
	ENDR

.done:
	jr .done
