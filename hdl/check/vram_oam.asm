; SPDX-License-Identifier: MIT
;
; VRAM and OAM read back what is written to them, and the PPU locks them
; out as it should. With the LCD on: written and read back in VBlank; VRAM
; reads FF in mode 3 and a write to it there is lost; OAM reads FF in mode
; 3 and in mode 2 (on a DMG, the read in mode 2 may corrupt OAM: the OAM
; bug). Then, with the LCD off, every byte of VRAM (8000 to 9FFF) and OAM
; (FE00 to FE9F) is written and read back, in two patterns (the address's
; bytes XORed, and with A5). It reports as report.inc says, so it runs on a
; Game Boy too, and on the model (0.5 s; a trace of the reads with the LCD
; on needs only the first 0.1 s, as the whole run's would pass 1 GiB):
;
;   ./run-tests.sh vram_oam       (dmg-sim; MODEL=1: the model)
;   TEST=mooneye ./trace-blocks.sh hdl/build/vram_oam.gb 0.1
;
;   rgbasm -I check/ -o vram_oam.o vram_oam.asm && rgblink -o vram_oam.gb vram_oam.o && rgbfix -v -p 0 vram_oam.gb

INCLUDE "report.inc"

DEF VRAM_BYTE EQU $8123
DEF MAP_BYTE EQU $9A10
DEF OAM_BYTE EQU $FE05

SECTION "entry", ROM0[$0100]
	nop
	jp Main

SECTION "main", ROM0[$0150]
Main:
	di
	ld sp, $FFFE
	xor a
	ldh [$FFFF], a              ; IE: nothing
	ld a, $91
	ldh [$FF40], a              ; the LCD on (if the boot left it off)

	; In VBlank, VRAM and OAM are the CPU's.
	call WaitVblank
	ld hl, VRAM_BYTE
	ld d, $C3
	call WriteAndCheck
	ld hl, MAP_BYTE
	ld d, $3C
	call WriteAndCheck
	ld hl, OAM_BYTE
	ld d, $5A
	call WriteAndCheck

	; In mode 3, VRAM reads FF, and a write to it is lost.
	call WaitMode3
	ld a, [VRAM_BYTE]
	ld hl, VRAM_BYTE
	ld d, $FF
	call Expect
	call WaitMode3
	xor a
	ld [VRAM_BYTE], a
	call WaitVblank
	ld a, [VRAM_BYTE]
	ld hl, VRAM_BYTE
	ld d, $C3
	call Expect

	; In mode 3, OAM reads FF.
	call WaitMode3
	ld a, [OAM_BYTE]
	ld hl, OAM_BYTE
	ld d, $FF
	call Expect

	; In mode 2, OAM reads FF: inline, as mode 2 is short (20 M-cycles).
	call WaitVblank
.not_mode2:
	ldh a, [$FF41]
	and 3
	cp 2
	jr z, .not_mode2
.mode2:
	ldh a, [$FF41]
	and 3
	cp 2
	jr nz, .mode2
	ld a, [OAM_BYTE]
	ld hl, OAM_BYTE
	ld d, $FF
	call Expect

	; With the LCD off, every byte (which also undoes what the read in mode
	; 2 may have done to OAM).
	call LcdOff
	ld c, $00
	call TestVram
	ld c, $A5
	call TestVram
	ld c, $00
	call TestOam
	ld c, $A5
	call TestOam
	jp CheckPassed

; Every byte of VRAM, written with its address's bytes XORed with C, then
; read back.
TestVram:
	ld hl, $8000
.fill:
	ld a, l
	xor h
	xor c
	ld [hl+], a
	ld a, h
	cp $A0
	jr nz, .fill
	ld hl, $8000
.check:
	ld a, l
	xor h
	xor c
	ld d, a
	ld a, [hl]
	call Expect
	inc hl
	ld a, h
	cp $A0
	jr nz, .check
	ret

; Every byte of OAM, the same way.
TestOam:
	ld hl, $FE00
.fill:
	ld a, l
	xor h
	xor c
	ld [hl+], a
	ld a, l
	cp $A0
	jr nz, .fill
	ld hl, $FE00
.check:
	ld a, l
	xor h
	xor c
	ld d, a
	ld a, [hl]
	call Expect
	inc hl
	ld a, l
	cp $A0
	jr nz, .check
	ret

; D written to HL and read back.
WriteAndCheck:
	ld a, d
	ld [hl], a
	ld a, [hl]
	; Fall through.

; A, read from HL, is D, or the test fails.
Expect:
	cp d
	ret z
	ld e, a
	jp CheckFailed

; Line 145, well inside VBlank.
WaitVblank:
	ldh a, [$FF44]
	cp 145
	jr nz, WaitVblank
	ret

; The start of a mode 3: out of mode 3 first, so that the wait does not
; end late in one.
WaitMode3:
	ldh a, [$FF41]
	and 3
	cp 3
	jr z, WaitMode3
.mode3:
	ldh a, [$FF41]
	and 3
	cp 3
	jr nz, .mode3
	ret
