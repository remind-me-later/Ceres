; SPDX-License-Identifier: MIT
;
; A stimulus for the APU's check (see ../README.md): it plays all four
; channels at once for about 70 ms and goes through most of what they can do:
; channel 1's sweep up to its overflow and down, the envelopes up, down and to
; their limits, the lengths up to their end, channel 3's wave RAM read while
; it plays and its restart, the noise's 15- and 7-bit widths and divisor 0, a
; write to DIV, the APU's power off and on, frequency writes while a channel
; plays, and the DACs turned off. It reports no result: dmg-sim runs it, and
; the readable model is checked against the signals it records.
;
;   rgbasm -o apu.o apu.asm && rgblink -o apu.gb apu.o && rgbfix -v -p 0 apu.gb

; A wait of about \1 ms (150 loops of 7 M-cycles).
MACRO wait_ms
	ld bc, (\1) * 150
	call delay
ENDM

; Writes \2 to the I/O register \1 (FF00 + \1).
MACRO out
	ld a, \2
	ldh [$FF00 + \1], a
ENDM

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	ld sp, $FFFE

	; The registers while the APU is off, then its power on.
	call read_all
	out $26, $80
	out $24, $77
	out $25, $F3
	call read_all

	; The wave: 00 11 22 .. FF, written while channel 3 is off.
	ld hl, $FF30
	xor a
.wave:
	ld [hl+], a
	add a, $11
	jr nc, .wave

	; Channel 1: duty 2, length 60 (4 steps), volume 15 going down every
	; step, frequency 500 going up by a half every sweep step: 780 at the
	; first, and then the next would overflow, which stops it before its
	; length ends.
	out $10, $11
	out $11, $BC
	out $12, $F1
	out $13, $00
	out $14, $C5

	; Channel 2: duty 1, volume 0 going up every step, no length.
	out $16, $40
	out $17, $09
	out $18, $80
	out $19, $87

	; Channel 3: full volume, frequency 700, no length.
	out $1A, $80
	out $1C, $20
	out $1D, $00
	out $1E, $87

	; Channel 4: length 60, volume 15 going down every step, the 15-bit
	; noise clocked by bit 2 of the prescaler, divisor 1.
	out $20, $3C
	out $21, $F1
	out $22, $21
	out $23, $C0

	; The registers and the wave RAM while all four play.
	call read_all
	call read_all
	wait_ms 4
	call read_all

	; Channel 3 quieter, channel 4's 7-bit noise with divisor 0, and
	; channel 2's frequency changed without a restart.
	out $1C, $40
	out $22, $08
	out $18, $10
	out $19, $06
	wait_ms 5
	out $1C, $60
	call read_all

	; A write to DIV moves the frame sequencer's steps.
	ldh [$FF04], a
	wait_ms 6
	call read_all

	; Channel 1 again, with length 60, sweeping down by an eighth every
	; step; channel 3 restarted while it plays, with length 252 (4 steps).
	out $10, $1B
	out $11, $BC
	out $12, $A9
	out $13, $F0
	out $14, $C7
	out $1B, $FC
	out $1E, $C7
	call read_all
	wait_ms 8

	; Channel 1's frequency written while it sweeps; channel 2 restarted
	; with volume 2 going down every step, which stops at 0.
	out $13, $55
	out $14, $45
	out $17, $21
	out $19, $C7
	wait_ms 10
	call read_all
	out $17, $29     ; written while it plays
	wait_ms 10
	call read_all

	; The power off: the registers are cleared and ignore writes but the
	; lengths; then on again, and channel 4 restarted with a length.
	out $26, $00
	out $11, $3E
	out $12, $F0
	out $20, $3E
	call read_all
	out $26, $80
	out $25, $FF
	out $21, $F0
	out $22, $10
	out $23, $80
	out $23, $40
	call read_all
	wait_ms 6

	; The DACs off.
	out $12, $00
	out $1A, $00
	out $21, $07
	call read_all
	wait_ms 3

.end:
	jr .end

; Reads FF10 to FF3F.
read_all:
	ld c, $10
.read:
	ldh a, [c]
	inc c
	ld a, c
	cp $40
	jr nz, .read
	ret

; Waits BC loops of 7 M-cycles.
delay:
	dec bc
	ld a, b
	or c
	jr nz, delay
	ret
