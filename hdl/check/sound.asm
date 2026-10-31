; SPDX-License-Identifier: MIT
;
; The APU's registers read back, and its channels do what NR52 shows. Each
; register (FF10 to FF25, without NR52) reads its bits back after writes of
; 00 and FF, with its unused and write-only bits high; NR52 reads 70 with
; the APU off, whose registers read cleared, and F0 once it is on again.
; Each channel's length runs out in two steps (NRx1 62, NR31 254): NR52's
; bit for it is set after its trigger and clear 20 ms later. (Not one step:
; a trigger that enables the length steps it at once when the frame
; sequencer's next step does not, and a length of 0 is then reloaded to its
; whole.) Channel 1's
; sweep overflows as it triggers (frequency 7FF, shift 1: NR52's bit 0
; clears at once), and updates for 20 ms without overflowing. The rest
; only plays, for the replay of the model's traces to cover it: envelopes
; that reach 15 and 0, and change direction while they play; every duty of
; channels 1 and 2; channel 3's levels; channel 4's clocks, its short
; LFSR, and the shift that stops its clock. It reports as report.inc says,
; so it runs on a Game Boy too (FF80 holds the test that failed), and on
; the model (0.26 s from the fast boot ROM, which trace-blocks.sh takes):
;
;   ./run-tests.sh sound          (dmg-sim; MODEL=1: the model)
;   TEST=mooneye ./trace-blocks.sh hdl/build/sound.gb 0.26
;
;   rgbasm -I check/ -o sound.o sound.asm && rgblink -o sound.gb sound.o && rgbfix -v -p 0 sound.gb

INCLUDE "report.inc"

; The test running (1 to 8), for a failure's report.
DEF TEST EQU $FF80

SECTION "entry", ROM0[$0100]
	nop
	jp Main

SECTION "main", ROM0[$0150]
Main:
	di
	ld sp, $DFFE
	call TestRegisters
	call TestPower
	call TestLengths
	call TestSweep
	call TestEnvelopes
	call TestDuties
	call TestWave
	call TestNoise
	jp CheckPassed

; The APU off and on: every register cleared, all to both sides, the
; volume full.
PowerCycle:
	xor a
	ldh [$FF26], a
	ld a, $80
	ldh [$FF26], a
	ld a, $77
	ldh [$FF24], a
	ld a, $FF
	ldh [$FF25], a
	ret

; Each register reads back 00 and FF, its unused and write-only bits high.
TestRegisters:
	ld a, 1
	ldh [TEST], a
	call PowerCycle
	ld hl, Registers
.register:
	ld a, [hl+]
	and a
	ret z
	ld c, a
	ld a, [hl+]
	ld b, a                     ; the bits that read high
	xor a
	ldh [c], a
	ldh a, [c]
	ld d, b
	call ExpectAtC
	ld a, $FF
	ldh [c], a
	ldh a, [c]
	ld d, $FF
	call ExpectAtC
	jr .register

; The APU off: NR52 reads 70 and its registers cleared; on again, F0.
TestPower:
	ld a, 2
	ldh [TEST], a
	ld a, $7F
	ldh [$FF10], a
	ld a, $F3
	ldh [$FF12], a
	ld a, $80
	ldh [$FF1A], a
	xor a
	ldh [$FF26], a              ; off
	ld c, $26
	ld d, $70
	call ExpectRead
	ld c, $10
	ld d, $80
	call ExpectRead
	ld c, $12
	ld d, $00
	call ExpectRead
	ld c, $1A
	ld d, $7F
	call ExpectRead
	ld c, $24
	ld d, $00
	call ExpectRead
	ld c, $25
	ld d, $00
	call ExpectRead
	ld a, $80
	ldh [$FF26], a              ; on
	ld c, $26
	ld d, $F0
	jp ExpectRead

; Each channel's length runs out in two steps of 256 Hz.
TestLengths:
	ld a, 3
	ldh [TEST], a
	call PowerCycle
	ld a, $3E
	ldh [$FF11], a              ; channel 1: length 62
	ld a, $F0
	ldh [$FF12], a
	xor a
	ldh [$FF13], a
	ld a, $C7
	ldh [$FF14], a              ; trigger, with the length
	ld d, $01
	call ExpectLengthRunsOut

	ld a, $3E
	ldh [$FF16], a              ; channel 2
	ld a, $F0
	ldh [$FF17], a
	xor a
	ldh [$FF18], a
	ld a, $C7
	ldh [$FF19], a
	ld d, $02
	call ExpectLengthRunsOut

	ld a, $80
	ldh [$FF1A], a              ; channel 3: its DAC on, length 254
	ld a, $FE
	ldh [$FF1B], a
	ld a, $20
	ldh [$FF1C], a
	xor a
	ldh [$FF1D], a
	ld a, $C7
	ldh [$FF1E], a
	ld d, $04
	call ExpectLengthRunsOut

	ld a, $3E
	ldh [$FF20], a              ; channel 4
	ld a, $F0
	ldh [$FF21], a
	xor a
	ldh [$FF22], a
	ld a, $C0
	ldh [$FF23], a
	ld d, $08
	; Fall through.

; NR52's bit D is set, then clear 20 ms later.
ExpectLengthRunsOut:
	push de
	call ExpectStatus
	ld e, 20
	call Delay
	pop de
	ld d, $00
	; Fall through.

; NR52's channel bits are D.
ExpectStatus:
	ldh a, [$FF26]
	and $0F
	ld hl, $FF26
	jp Expect

; Channel 1's sweep: an overflow as it triggers stops it at once; a sum
; that does not overflow updates the frequency for 20 ms; then it
; subtracts for 10 ms.
TestSweep:
	ld a, 4
	ldh [TEST], a
	call PowerCycle
	ld a, $11
	ldh [$FF10], a              ; period 1, adding, shift 1
	ld a, $80
	ldh [$FF11], a
	ld a, $F0
	ldh [$FF12], a
	ld a, $FF
	ldh [$FF13], a
	ld a, $87
	ldh [$FF14], a              ; frequency 7FF: 7FF + 3FF overflows
	ld e, 1
	call Delay
	ld d, $00
	call ExpectStatus

	xor a
	ldh [$FF13], a
	ld a, $81
	ldh [$FF14], a              ; frequency 100: 100, 180, 240, 360
	ld e, 20
	call Delay
	ld d, $01
	call ExpectStatus
	ld a, $19
	ldh [$FF10], a              ; subtracting
	ld e, 10
	call Delay
	ld d, $01
	jp ExpectStatus

; Envelopes that reach 15 (channel 2, from 14) and 0 (channel 4, from 1),
; and change direction while they play (channel 1).
TestEnvelopes:
	ld a, 5
	ldh [TEST], a
	call PowerCycle
	ld a, $E9
	ldh [$FF17], a              ; 14, up, period 1
	ld a, $87
	ldh [$FF19], a
	ld a, $19
	ldh [$FF21], a              ; 1, down, period 1
	ld a, $80
	ldh [$FF23], a
	ld a, $81
	ldh [$FF12], a              ; 8, down, period 1
	ld a, $87
	ldh [$FF14], a
	ld e, 20
	call Delay
	ld a, $89
	ldh [$FF12], a              ; up
	ld a, $81
	ldh [$FF12], a              ; down
	ld a, $88
	ldh [$FF12], a              ; up, no period
	ld e, 20
	jp Delay

; Every duty of channels 1 and 2, 2 ms each, at 2 KiHz.
TestDuties:
	ld a, 6
	ldh [TEST], a
	call PowerCycle
	ld a, $F0
	ldh [$FF12], a
	ldh [$FF17], a
	ld a, $C0
	ldh [$FF13], a
	ldh [$FF18], a
	ld a, $87
	ldh [$FF14], a
	ldh [$FF19], a
	ld d, $00                   ; the duty (Delay takes BC)
.duty:
	ld a, d
	ldh [$FF11], a
	ldh [$FF16], a
	ld e, 2
	call Delay
	ld a, d
	add $40
	ld d, a
	jr nz, .duty
	ret

; Channel 3 at each level, from wave RAM, 2 ms each.
TestWave:
	ld a, 7
	ldh [TEST], a
	call PowerCycle
	ld hl, $FF30
	ld a, $01
.fill:
	ld [hl+], a
	add $23
	bit 6, l
	jr z, .fill
	ld a, $80
	ldh [$FF1A], a
	ld a, $F0
	ldh [$FF1D], a
	ld a, $87
	ldh [$FF1E], a
	ld d, $20                   ; the level (Delay takes BC)
.level:
	ld a, d
	ldh [$FF1C], a
	ld e, 2
	call Delay
	ld a, d
	add $20
	ld d, a
	cp $80
	jr nz, .level
	ret

; Channel 4's clocks, 2 ms each: NR43's values from the table.
TestNoise:
	ld a, 8
	ldh [TEST], a
	call PowerCycle
	ld a, $F0
	ldh [$FF21], a
	ld hl, NoiseClocks
.clock:
	ld a, [hl+]
	cp $FF
	ret z
	ldh [$FF22], a
	ld a, $80
	ldh [$FF23], a
	ld e, 2
	call Delay
	jr .clock

; Waits about E milliseconds.
Delay:
.ms:
	ld bc, 149
.loop:
	dec bc
	ld a, b
	or c
	jr nz, .loop
	dec e
	jr nz, .ms
	ret

; FF00 + C reads D.
ExpectRead:
	ldh a, [c]
	; Fall through.

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

; The registers' low bytes and the bits that read high, 00 last.
Registers:
	db $10, $80, $11, $3F, $12, $00, $13, $FF, $14, $BF
	db $15, $FF, $16, $3F, $17, $00, $18, $FF, $19, $BF
	db $1A, $7F, $1B, $FF, $1C, $9F, $1D, $FF, $1E, $BF
	db $1F, $FF, $20, $FF, $21, $00, $22, $00, $23, $BF
	db $24, $00, $25, $00
	db $00
; NR43's values: the dividers, the short LFSR, the shifts (14 stops the
; clock), FF last.
NoiseClocks:
	db $00, $01, $08, $11, $27, $3D, $D0, $E0, $F7, $FF
