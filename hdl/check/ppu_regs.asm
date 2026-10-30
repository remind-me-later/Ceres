; SPDX-License-Identifier: MIT
;
; The PPU's registers read back what is written to them: SCY, SCX, LYC,
; BGP, OBP0, OBP1, WY and WX every bit, with the LCD on and off; LCDC every
; bit (the LCD stays on, then off); STAT its enables (bits 3 to 6), with
; bit 7 high; FF46 the source, while OAM DMA runs and after. It reports as
; report.inc says, so it runs on a Game Boy too, and on the model:
;
;   ./run-tests.sh ppu_regs       (dmg-sim; MODEL=1: the model)
;   TEST=mooneye ./trace-blocks.sh hdl/build/ppu_regs.gb 0.3
;
;   rgbasm -I check/ -o ppu_regs.o ppu_regs.asm && rgblink -o ppu_regs.gb ppu_regs.o && rgbfix -v -p 0 ppu_regs.gb

INCLUDE "report.inc"

DEF NUM_PATTERNS EQU 12

SECTION "entry", ROM0[$0100]
	nop
	jp Main

SECTION "main", ROM0[$0150]
Main:
	di
	ld sp, $FFFE
	xor a
	ldh [$FFFF], a              ; IE: nothing
	; The DMA routine, in HRAM: the CPU reads only HRAM while it runs.
	ld hl, DmaRoutine
	ld de, $FF80
	ld b, DmaRoutineEnd - DmaRoutine
.copy:
	ld a, [hl+]
	ld [de], a
	inc e
	dec b
	jr nz, .copy

	ld a, $91
	ldh [$FF40], a              ; the LCD on
	call TestRegisters
	call TestLcdcOn
	call TestStat
	call TestDma
	call LcdOff
	call TestRegisters
	call TestLcdcOff
	call TestStat
	call TestDma
	jp CheckPassed

; Every pattern written to every register of the table, and read back.
TestRegisters:
	ld hl, Registers
.register:
	ld a, [hl+]
	and a
	ret z
	ld c, a
	push hl
	ld hl, Patterns
	ld b, NUM_PATTERNS
.pattern:
	ld a, [hl+]
	ld d, a
	ldh [c], a
	ldh a, [c]
	ld e, a
	cp d
	jr nz, RegisterFailed
	dec b
	jr nz, .pattern
	pop hl
	jr .register

; The register at C failed: D written, E read.
RegisterFailed:
	ld h, $FF
	ld l, c
	jp CheckFailed

; LCDC with the LCD on: every pattern with bit 7 set.
TestLcdcOn:
	ld c, $40
	ld hl, Patterns
	ld b, NUM_PATTERNS
.pattern:
	ld a, [hl+]
	or $80
	ld d, a
	ldh [c], a
	ldh a, [c]
	ld e, a
	cp d
	jr nz, RegisterFailed
	dec b
	jr nz, .pattern
	ld a, $91
	ldh [c], a
	ret

; LCDC with the LCD off: every pattern with bit 7 clear.
TestLcdcOff:
	ld c, $40
	ld hl, Patterns
	ld b, NUM_PATTERNS
.pattern:
	ld a, [hl+]
	and $7F
	ld d, a
	ldh [c], a
	ldh a, [c]
	ld e, a
	cp d
	jr nz, RegisterFailed
	dec b
	jr nz, .pattern
	xor a
	ldh [c], a
	ret

; STAT: the enables read back, bit 7 reads high; the mode and LY = LYC's
; flag (bits 0 to 2) are left out.
TestStat:
	ld c, $41
	ld hl, Patterns
	ld b, NUM_PATTERNS
.pattern:
	ld a, [hl+]
	and $78
	or $80
	ld d, a
	ldh [c], a
	ldh a, [c]
	and $F8
	ld e, a
	cp d
	jr nz, RegisterFailed
	dec b
	jr nz, .pattern
	xor a
	ldh [c], a
	ret

; FF46 reads back the source, while the transfer runs and after it.
TestDma:
	ld hl, DmaSources
.source:
	ld a, [hl+]
	cp $FF
	ret z
	ld d, a
	push hl
	call $FF80
	pop hl
	ld a, b
	cp d
	jr nz, .failed
	ld a, c
	cp d
	jr nz, .failed
	jr .source
.failed:
	ld e, a
	ld c, $46
	jr RegisterFailed

; In HRAM: a transfer from D's page, FF46 read at once into B and after
; it into C.
DmaRoutine:
	ld a, d
	ldh [$FF46], a
	ldh a, [$FF46]
	ld b, a
	ld a, 40                    ; 160 M-cycles: the transfer's length
.wait:
	dec a
	jr nz, .wait
	ldh a, [$FF46]
	ld c, a
	ret
DmaRoutineEnd:

Registers:
	db $42, $43, $45, $47, $48, $49, $4A, $4B, 0
Patterns:
	db $00, $FF, $55, $AA, $01, $02, $04, $08, $10, $20, $40, $80
DmaSources:
	db $00, $80, $9F, $C0, $C1, $DF, $FF
