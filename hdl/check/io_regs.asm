; SPDX-License-Identifier: MIT
;
; The registers the chip's other blocks answer read back: IE keeps all 8
; bits; IF reads its flags with bits 7 to 5 high; TMA and TIMA read back
; while the timer is stopped, TAC with bits 7 to 3 high; TIMA counts once
; 256 M-cycles after a write of DIV with TAC 4; DIV reads 00 right after a
; write and 03 some 224 M-cycles later; the I/O addresses nothing decodes
; (FF03, FF08 to FF0E, FF4C to FF4F, FF51 to FF7F, as Mooneye's
; unused_hwio-GS has them) read FF after writes of FF and 00 (FF60, the test
; modes' register, is only read). The LCD is off
; and no transfer runs, so nothing but the timer raises IF. It reports as
; report.inc says, so it runs on a Game Boy too (press nothing while it
; runs), and on the model (0.4 s with the quick boot ROM; it ends in 71 ms
; with the fast one, which trace-blocks.sh takes):
;
;   ./run-tests.sh io_regs        (dmg-sim; MODEL=1: the model)
;   TEST=mooneye ./trace-blocks.sh hdl/build/io_regs.gb 0.1
;
;   rgbasm -I check/ -o io_regs.o io_regs.asm && rgblink -o io_regs.gb io_regs.o && rgbfix -v -p 0 io_regs.gb

INCLUDE "report.inc"

SECTION "entry", ROM0[$0100]
	nop
	jp Main

SECTION "main", ROM0[$0150]
Main:
	di
	ld sp, $DFFE
	call LcdOff                 ; no VBlank or STAT requests
	call TestIe
	call TestIf
	call TestTimer
	call TestDiv
	call TestUnused
	jp CheckPassed

; IE: every pattern reads back.
TestIe:
	ld hl, Patterns
.pattern:
	ld a, [hl+]
	ld d, a
	ldh [$FFFF], a
	ldh a, [$FFFF]
	push hl
	ld hl, $FFFF
	call Expect
	pop hl
	ld a, d
	cp $FF
	jr nz, .pattern
	xor a
	ldh [$FFFF], a
	ret

; IF: the five flags read back, bits 7 to 5 high.
TestIf:
	ld hl, Patterns
.pattern:
	ld a, [hl+]
	ld e, a
	ldh [$FF0F], a
	or $E0
	ld d, a
	ldh a, [$FF0F]
	push hl
	ld hl, $FF0F
	call Expect
	pop hl
	ld a, e
	cp $FF
	jr nz, .pattern
	xor a
	ldh [$FF0F], a
	ret

; TMA and TIMA while the timer is stopped, TAC with its unused bits high;
; then TIMA counts once in 384 M-cycles with TAC 4 (every 256 M-cycles,
; from a write of DIV).
TestTimer:
	xor a
	ldh [$FF07], a              ; TAC: stopped
	ld hl, Patterns
.pattern:
	ld a, [hl+]
	ld d, a
	ld e, a
	ldh [$FF06], a              ; TMA
	ldh a, [$FF06]
	push hl
	ld hl, $FF06
	call Expect
	ld a, d
	ldh [$FF05], a              ; TIMA
	ldh a, [$FF05]
	ld hl, $FF05
	call Expect
	ld a, d
	and $03                     ; TAC, stopped (bit 2 clear)
	ldh [$FF07], a
	or $F8
	ld d, a
	ldh a, [$FF07]
	ld hl, $FF07
	call Expect
	pop hl
	ld a, e
	cp $FF
	jr nz, .pattern

	xor a
	ldh [$FF07], a              ; stopped
	ldh [$FF05], a              ; TIMA 0
	ldh [$FF0F], a
	ldh [$FF04], a              ; DIV from 0
	ld a, $04
	ldh [$FF07], a              ; TAC: every 256 M-cycles
	ld b, 96
.wait:
	dec b
	jr nz, .wait
	ldh a, [$FF05]
	ld hl, $FF05
	ld d, $01
	call Expect
	ld a, $07
	ldh [$FF07], a              ; TAC 7 reads FF
	ld d, $FF
	ldh a, [$FF07]
	ld hl, $FF07
	call Expect
	xor a
	ldh [$FF07], a
	ldh a, [$FF0F]              ; no overflow
	ld hl, $FF0F
	ld d, $E0
	jp Expect

; DIV: 00 right after a write, 03 some 224 M-cycles later (it counts every
; 64).
TestDiv:
	ldh [$FF04], a
	ldh a, [$FF04]
	ld hl, $FF04
	ld d, $00
	call Expect
	ldh [$FF04], a
	ld b, 55
.wait:
	dec b
	jr nz, .wait
	ldh a, [$FF04]
	ld d, $03
	jp Expect

; The I/O addresses nothing decodes read FF, after writes of FF and 00;
; FF60, which the chip decodes for its test modes (it does not read it), is
; only read.
TestUnused:
	ld hl, Unused
.address:
	ld a, [hl+]
	and a
	jr z, .test_mode
	ld c, a
	ld a, $FF
	ldh [c], a
	ldh a, [c]
	ld d, $FF
	call ExpectAtC
	xor a
	ldh [c], a
	ldh a, [c]
	call ExpectAtC
	jr .address
.test_mode:
	ldh a, [$FF60]
	ld hl, $FF60
	jp Expect

; A, read from FF00 + C, is D.
ExpectAtC:
	push hl
	ld h, $FF
	ld l, c
	call Expect
	pop hl
	ret

; A, read from HL, is D, or the test fails.
Expect:
	cp d
	ret z
	ld e, a
	jp CheckFailed

; The patterns, FF last.
Patterns:
	db $00, $A5, $5A, $1F, $E0, $0F, $F0, $FF
; The I/O addresses nothing decodes (their low bytes), 00 last.
Unused:
	db $03, $08, $09, $0A, $0B, $0C, $0D, $0E
	db $4C, $4D, $4E, $4F
	db $51, $52, $53, $54, $55, $56, $57, $58, $59, $5A, $5B, $5C, $5D, $5E, $5F
	db $61, $62, $63, $64, $65, $66, $67, $68, $69, $6A, $6B, $6C, $6D, $6E, $6F
	db $70, $71, $72, $73, $74, $75, $76, $77, $78, $79, $7A, $7B, $7C, $7D, $7E, $7F
	db $00
