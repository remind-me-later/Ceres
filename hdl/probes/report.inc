; SPDX-License-Identifier: MIT
;
; How the check ROMs that verify what they read report their result, so
; that they run on the model (../run-tests.sh, TEST=mooneye), on ceres and
; on a Game Boy:
;
; - as Mooneye's tests: LD B,B with 3, 5, 8, 13, 21 and 34 in B, C, D, E, H
;   and L if they passed, all 42 if they failed;
; - as Blargg's: "Passed" or "Failed" on the serial port;
; - on the screen: white if they passed, black if they failed.
;
; A failure also leaves at C000 the address (low, high), the expected byte
; and the byte read.
;
; Jump to CheckFailed with HL the address, D the expected byte and E the byte
; read; to CheckPassed when everything held.

DEF RESULT EQU $C000

SECTION "report", ROM0

CheckFailed::
	ld a, l
	ld [RESULT], a
	ld a, h
	ld [RESULT + 1], a
	ld a, d
	ld [RESULT + 2], a
	ld a, e
	ld [RESULT + 3], a
	ld hl, FailedText
	ld b, $FF                   ; BGP: every shade black
	jr Report

CheckPassed::
	ld hl, PassedText
	ld b, $00                   ; BGP: every shade white

; The screen in BGP B, the text at HL on the serial port, the registers.
Report:
	push hl
	push bc
	call LcdOff
	; Tile 0 blank, and the map all tile 0.
	ld hl, $8000
	ld bc, 16
	call Clear
	ld hl, $9800
	ld bc, $400
	call Clear
	pop bc
	ld a, b
	ldh [$FF47], a
	xor a
	ldh [$FF42], a              ; SCY
	ldh [$FF43], a              ; SCX
	ld a, $91
	ldh [$FF40], a              ; the LCD and the background on
	pop hl
	ld a, [hl]
	ld c, a                     ; "P" or "F"
.serial:
	ld a, [hl+]
	and a
	jr z, .registers
	ldh [$FF01], a
	ld a, $81
	ldh [$FF02], a              ; send, with the internal clock
.sending:
	ldh a, [$FF02]
	bit 7, a
	jr nz, .sending
	jr .serial
.registers:
	ld a, c
	cp 'P'
	jr nz, .failed
	ld b, 3
	ld c, 5
	ld d, 8
	ld e, 13
	ld h, 21
	ld l, 34
	jr .done
.failed:
	ld a, $42
	ld b, a
	ld c, a
	ld d, a
	ld e, a
	ld h, a
	ld l, a
.done:
	ld b, b                     ; Mooneye's end of the test
.halt:
	jr .halt

; The LCD off, in VBlank (turning it off outside VBlank may harm a Game
; Boy's LCD); nothing if it is off already.
LcdOff::
	ldh a, [$FF40]
	bit 7, a
	ret z
.vblank:
	ldh a, [$FF44]
	cp 144
	jr nz, .vblank
	xor a
	ldh [$FF40], a
	ret

; BC bytes of zero from HL on.
Clear:
	xor a
	ld [hl+], a
	dec bc
	ld a, b
	or c
	jr nz, Clear
	ret

PassedText:
	db "Passed\n", 0
FailedText:
	db "Failed\n", 0
