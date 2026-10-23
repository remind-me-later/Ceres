; SPDX-License-Identifier: MIT
;
; A boot ROM that leaves the machine as dmg-sim's quickboot does, in about a
; ninth of its time (17.6 thousand M-cycles instead of 158 thousand), for the
; tests that run on the readable model, where each M-cycle costs seconds.
;
; Quickboot (external/reference-implementations/dmg-sim/boot/quickboot.s)
; zeroes the VRAM (98 thousand M-cycles), turns the LCD on, and leaves three
; frames and 16 M-cycles later, with DIV where the original boot ROM leaves it.
; This one starts with the VRAM zeroed (the board's VRAM_ZERO), leaves one
; frame and 16 M-cycles after the LCD, which is the same point of the frame,
; and writes DIV once, at the M-cycle that gives it the same phase at the exit.
; The registers, the sound registers, BGP, LCDC and the two bytes that
; quickboot's last CALL leaves on the stack are the same.
;
; Not the same: the first video frame is one, not three, the sound
; channel's envelope has had one frame to run, and the sound frame sequencer
; may not be in the same step. Tests of the sound should use quickboot.
;
; The times are M-cycles from the first fetch, as the bus trace counts them.
;
;   rgbasm -o tinyboot.o tinyboot.asm && rgblink -x -o tinyboot.bin tinyboot.o

; When the LCD is turned on, and when DIV is written (the phase of DIV is
; fixed by the difference of the exit and the write, modulo 16384).
DEF LCD_ON  EQU 300
DEF DIV_AT  EQU 300 + 6548 - 2
DEF EXIT_AT EQU LCD_ON + 17540

DEF cyc = 0

; Adds the M-cycles of the instruction just written.
MACRO cy
	DEF cyc += \1
ENDM

MACRO nops
	REPT \1
		nop
	ENDR
	DEF cyc += \1
ENDM

; Waits until the next instruction starts in M-cycle \1 (a loop of DEC DE,
; LD A,D, OR E, JR NZ: 7 M-cycles a turn, after LD DE, n: 3).
MACRO wait_to
	DEF pad = (\1) - cyc
	IF pad < 0
		FAIL "wait_to: too late"
	ENDC
	IF pad >= 24
		DEF turns = (pad - 3 + 1) / 7
		ld de, turns
		wait_loop\@:
		dec de
		ld a, d
		or e
		jr nz, wait_loop\@
		DEF cyc += 3 + 7 * turns - 1
	ENDC
	nops (\1) - cyc
ENDM

; Writes the value \3 to the register \2 in M-cycle \1 (LDH [n], A writes in
; its third; LD A, n takes two before it).
MACRO write_at
	wait_to (\1) - 4
	ld a, \3
	cy 2
	ldh [\2], a
	cy 3
ENDM

SECTION "boot", ROM0[$0000]
	ld sp, $FFFE
	cy 3

	; The sound, first part, as quickboot has it.
	ld a, $80
	cy 2
	ldh [$FF26], a
	cy 3
	ldh [$FF11], a
	cy 3
	ld a, $11
	cy 2
	ldh [$FF12], a
	cy 3
	ld a, $C1
	cy 2
	ldh [$FF13], a
	cy 3
	ld a, $87
	cy 2
	ldh [$FF14], a
	cy 3

	; BGP, and the LCD on.
	ld a, $FC
	cy 2
	ldh [$FF47], a
	cy 3
	write_at LCD_ON, $FF40, $91

	; The sound, second part.
	ld a, $F3
	cy 2
	ldh [$FF12], a
	cy 3
	ldh [$FF25], a
	cy 3
	ld a, $77
	cy 2
	ldh [$FF24], a
	cy 3

	; DIV.
	write_at DIV_AT, $FF04, 0

	; What quickboot's last CALL leaves on the stack.
	ld a, $4C
	cy 2
	ldh [$FFFC], a
	cy 3
	ld a, 0
	cy 2
	ldh [$FFFD], a
	cy 3

	; The exit: the registers, from the same instructions as quickboot's (A = 0
	; and F = Z, H, C), then LD A, 1 and the write to the boot ROM's off switch
	; are the last 17 bytes (EF to FF), so that the next fetch is at 0100. What
	; is left of the time is spent before them in a loop and in padding: 1-cycle
	; NOPs, and as many 2-cycle INC DEs as the loop's turns leave over.
	DEF tail_bytes = 13
	DEF start_pad = @
	DEF pad_bytes = $FC - tail_bytes - (start_pad + 8)
	DEF total = EXIT_AT - 4 - tail_bytes - cyc
	DEF turns = (total - pad_bytes - 3 + 1) / 7
	DEF extra = total - pad_bytes - (3 + 7 * turns - 1)
	ASSERT pad_bytes >= 0 && extra >= 0 && extra <= pad_bytes, "the exit does not fit"
	ld de, turns
exit_loop:
	dec de
	ld a, d
	or e
	jr nz, exit_loop
	REPT extra
		inc de
	ENDR
	REPT pad_bytes - extra
		nop
	ENDR
	ld a, $FF
	add a, 1
	ld bc, $0013
	ld de, $00D8
	ld hl, $014D
	ASSERT @ == $FC, "the exit is not at FC"
	ld a, 1
	ldh [$FF50], a
