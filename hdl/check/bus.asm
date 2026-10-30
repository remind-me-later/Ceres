; SPDX-License-Identifier: MIT
;
; The bus side reads back what it should: the serial port sends with its
; internal clock (nothing at the other end: SIN is high, SB reads FF after
; the transfer, SC reads 7F, IF's serial bit rises); P1's select bits read
; back, its unused bits high, no button pressed; FF50 reads FF once the boot
; ROM is unmapped; the cartridge's header reads back; every byte of HRAM
; (FF80 to FFFE); the echo of work RAM (E000 to FDFF) reads and writes C000
; to DDFF; then every byte of work RAM (C000 to DFFF), in two patterns (the
; address's bytes XORed, and with A5). The joypad's interrupt needs a button
; pressed, and is left out. It reports as report.inc says, so it runs on a
; Game Boy too (press nothing while it runs), and on the model (0.6 s; a
; trace of all but the work RAM's sweep needs only 0.1 s):
;
;   ./run-tests.sh bus            (dmg-sim; MODEL=1: the model)
;   TEST=mooneye ./trace-blocks.sh hdl/build/bus.gb 0.1
;
;   rgbasm -I check/ -o bus.o bus.asm && rgblink -o bus.gb bus.o && rgbfix -v -p 0 bus.gb

INCLUDE "report.inc"

SECTION "entry", ROM0[$0100]
	nop
	jp Main

SECTION "main", ROM0[$0150]
Main:
	di
	ld sp, $DFFE                ; the stack in work RAM, while HRAM is tested
	xor a
	ldh [$FFFF], a              ; IE: nothing

	call TestSerial
	call TestJoypad
	; FF50: the boot ROM is unmapped.
	ldh a, [$FF50]
	ld hl, $FF50
	ld d, $FF
	call Expect
	call TestHeader
	ld c, $00
	call TestHram
	ld c, $A5
	call TestHram
	call TestEcho

	; Work RAM, with the stack in HRAM.
	ld sp, $FFFE
	ld c, $00
	call TestWorkRam
	ld c, $A5
	call TestWorkRam
	jp CheckPassed

; Bytes sent with the internal clock: SB reads FF after each (SIN is high),
; SC 7F, and IF's serial bit rises.
TestSerial:
	ld hl, SerialBytes
.byte:
	ld a, [hl+]
	cp $FF
	ret z
	push hl
	ldh [$FF01], a              ; SB
	xor a
	ldh [$FF0F], a              ; IF clear
	ld a, $81
	ldh [$FF02], a              ; send, with the internal clock
.sending:
	ldh a, [$FF02]
	bit 7, a
	jr nz, .sending
	ld hl, $FF02
	ld d, $7F
	call Expect
	ldh a, [$FF01]
	ld hl, $FF01
	ld d, $FF
	call Expect
	ldh a, [$FF0F]
	and $08
	ld hl, $FF0F
	ld d, $08
	call Expect
	pop hl
	jr .byte

; P1: bits 7 and 6 read high, the select bits back, and the buttons (none
; pressed) high.
TestJoypad:
	ld hl, JoypadSelects
.select:
	ld a, [hl+]
	cp $FF
	ret z
	ld d, a
	ldh [$FF00], a
	ldh a, [$FF00]
	ld e, a
	ld a, d
	and $30
	or $CF
	ld d, a
	ld a, e
	push hl
	ld hl, $FF00
	call Expect
	pop hl
	jr .select

; The cartridge's header: the logo's first bytes, and the entry point.
TestHeader:
	ld hl, $0104
	ld d, $CE
	ld a, [hl]
	call Expect
	ld hl, $0105
	ld d, $ED
	ld a, [hl]
	call Expect
	ld hl, $0100
	ld d, $00                   ; nop
	ld a, [hl]
	jp Expect

; Every byte of HRAM, written with its address's low byte XORed with C,
; then read back.
TestHram:
	ld hl, $FF80
.fill:
	ld a, l
	xor c
	ld [hl+], a
	ld a, l
	cp $FF
	jr nz, .fill
	ld hl, $FF80
.check:
	ld a, l
	xor c
	ld d, a
	ld a, [hl]
	call Expect
	inc hl
	ld a, l
	cp $FF
	jr nz, .check
	ret

; The echo: a byte every 0x101 written through E000 to FDFF reads back at
; C000 to DDFF, and the other way round.
TestEcho:
	ld hl, $E000
.through_echo:
	ld a, l
	xor h
	ld [hl], a
	ld d, a
	push hl
	ld a, h
	sub $20
	ld h, a                     ; the same byte in work RAM
	ld a, [hl]
	call Expect
	cpl
	ld [hl], a                  ; and written there, read in the echo
	ld d, a
	pop hl
	ld a, [hl]
	call Expect
	ld bc, $0101
	add hl, bc
	ld a, h
	cp $FE
	jr c, .through_echo
	ret

; Every byte of work RAM, written with its address's bytes XORed with C,
; then read back.
TestWorkRam:
	ld hl, $C000
.fill:
	ld a, l
	xor h
	xor c
	ld [hl+], a
	ld a, h
	cp $E0
	jr nz, .fill
	ld hl, $C000
.check:
	ld a, l
	xor h
	xor c
	ld d, a
	ld a, [hl]
	call Expect
	inc hl
	ld a, h
	cp $E0
	jr nz, .check
	ret

; A, read from HL, is D, or the test fails.
Expect:
	cp d
	ret z
	ld e, a
	jp CheckFailed

SerialBytes:
	db $00, $55, $AA, $81, $FF
JoypadSelects:
	db $30, $20, $10, $00, $FF
