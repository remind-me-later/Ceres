; SPDX-License-Identifier: MIT
;
; Whether turning the LCD on with LYC 0 raises the STAT interrupt in the
; LCDC write's M-cycle or the one before (ceres-core's netlist_lcd_on). The
; LCD goes off in line 50, so that LY = LYC's flag is clear; with LYC's
; source enabled, turning it on compares LY 0 with LYC 0. With IME set and
; STAT's interrupt enabled, the CPU enters the handler after one of the INC
; Bs that follow the write: the return address it pushes, and B, tell
; which. Compare the model's accesses with ceres's (../compare-ceres.sh):
;
;   NOFRAMES=1 ./compare-ceres.sh hdl/build/lcd_on_lyc0.gb 0.05
;
;   rgbasm -o lcd_on_lyc0.o lcd_on_lyc0.asm && rgblink -o lcd_on_lyc0.gb lcd_on_lyc0.o && rgbfix -v -p 0 lcd_on_lyc0.gb

SECTION "stat", ROM0[$0048]
	ld c, b
	ld b, b
stat_halt:
	jr stat_halt

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	xor a
	ldh [$FFFF], a            ; IE: nothing, IF only
	ldh [$FF45], a            ; LYC 0
	ld b, 50
.wait:
	ldh a, [$FF44]
	cp b
	jr nz, .wait
	xor a
	ldh [$FF40], a            ; the LCD off in line 50: the flag stays clear
	ld a, $40
	ldh [$FF41], a            ; LYC's source
	xor a
	ldh [$FF0F], a
	ld a, 2
	ldh [$FFFF], a            ; IE: STAT's
	ld b, 0
	ei
	nop
	ld a, $91
	ldh [$FF40], a            ; the LCD on: LY 0 = LYC 0
	inc b
	inc b
	inc b
	inc b
.wait_int:
	jr .wait_int
