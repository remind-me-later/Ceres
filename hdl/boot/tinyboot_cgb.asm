; SPDX-License-Identifier: MIT
;
; A boot ROM that leaves the CGB as the CGB-E's boot ROM does, in about a
; hundredth of its time (35 thousand M-cycles instead of 3.26 million), for
; the tests that run on the CGB's model, where each M-cycle costs seconds.
;
; The CGB-E's boot ROM (external/gb-bootroms/bin/cgbE.bin) turns the LCD on
; at M-cycle 173,617 (173,616 for a cartridge without the CGB flag), and
; leaves at 3,262,374 (3,262,876), with the divider at 1960 (2462), modulo
; 16384, as ceres runs it and the model too (the two bus traces are the same).
; This one starts with the VRAM zeroed (the board's VRAM_ZERO), turns the LCD
; on, and leaves one frame later at the same point of the frame, and writes
; DIV once, at the M-cycle that gives it the same phase at the exit. As the
; real one, it writes KEY0 (the cartridge's CGB flag, or 04 and OPRI 01 for
; the DMG's mode), the palettes it leaves (all white for the background, and
; the default palettes of the DMG's mode: those of a game it does not know),
; and the registers.
;
; Not the same: the logo is not in VRAM nor in high RAM, the palettes of the
; DMG's mode are always the default ones, and the sound has had less time.
; Tests of those should use the real boot ROM.
;
; The times are M-cycles from the first fetch, as the bus trace counts them.
;
;   rgbasm -o tinyboot_cgb.o tinyboot_cgb.asm && rgblink -x -o tinyboot_cgb.bin tinyboot_cgb.o
;   (then padded to 2304 bytes)

DEF FRAME   EQU 17556
DEF LCD_ON  EQU 900

; From the LCD on to the exit (the fetch at 0100), and the divider at the
; exit, modulo 16384: the CGB's mode, then the DMG's.
DEF EXIT_C  EQU LCD_ON + FRAME + (3262374 - 173617) % FRAME
DEF EXIT_D  EQU LCD_ON - 1 + FRAME + (3262876 - 173616) % FRAME
DEF DIV_C   EQU 1960
DEF DIV_D   EQU 2462

; The tail at 00F9 (XOR A, LDH [70],A, LD A,11, LDH [50],A) and the jump to
; it take this long before the fetch at 0100.
DEF TAIL    EQU 4 + 9

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

; A = FF if A is \1, 00 otherwise.
MACRO equal
	sub \1
	cp 1
	sbc a, a
	cy 5
ENDM

; How long the DMG mode's registers take, before the tail.
DEF DMG_REGS EQU 216

; Writes A to the register \1.
MACRO put
	ldh [\1], a
	cy 3
ENDM

; The sound, the wave RAM and BGP, as the CGB-E's boot ROM leaves them.
MACRO sound
	ld a, $80
	cy 2
	put $FF26
	put $FF11
	ld a, $F3
	cy 2
	put $FF12
	put $FF25
	ld a, $77
	cy 2
	put $FF24
	FOR I, 16
		IF I % 2
			ld a, $FF
		ELSE
			xor a
		ENDC
		cy 2 - (I % 2 == 0)
		put $FF30 + I
	ENDR
	ld a, $C1
	cy 2
	put $FF13
	ld a, $87
	cy 2
	put $FF14
	ld a, $FC
	cy 2
	put $FF47
ENDM

; The 32 colours of the background's palettes, white (7FFF), from BCPS 80.
MACRO white_background
	ld a, $80
	cy 2
	put $FF68
	ld b, 32
	cy 2
.white\@
	ld a, $FF
	ldh [$FF69], a
	ld a, $7F
	ldh [$FF69], a
	dec b
	jr nz, .white\@
	; 2 + 3 + 2 + 3 + 1 + 3 a turn, 2 for the last JR
	DEF cyc += 14 * 32 - 1
ENDM

; Writes the bytes \2... to the palette data register \1, from its index
; register's current value.
MACRO colours
	DEF reg = \1
	SHIFT
	REPT _NARG
		ld a, \1
		cy 2
		put reg
		SHIFT
	ENDR
ENDM

SECTION "boot", ROM0[$0000]
	ld sp, $FFFE
	cy 3
	ld a, [$0143]
	cy 4
	bit 7, a
	cy 2
	jp z, dmg_mode
	cy 3
	jp cgb_mode

	; The tail, as the real boot ROM has it: the work RAM's bank, then the
	; boot ROM's off switch, so that the next fetch is at 0100.
	ds $F9 - @, 0
tail:
	xor a
	ldh [$FF70], a
	ld a, $11
	ldh [$FF50], a

SECTION "modes", ROM0[$0200]
cgb_mode:
	DEF cyc += 4
	sound
	white_background
	ld a, $80
	cy 2
	put $FF6A
	xor a
	cy 1
	put $FF6B
	write_at LCD_ON, $FF40, $91
	write_at EXIT_C - DIV_C, $FF04, 0
	wait_to EXIT_C - TAIL - 16
	; KEY0 takes the cartridge's CGB flag.
	ld a, [$0143]
	cy 4
	put $FF4C
	ld bc, $0000
	cy 3
	ld de, $FF56
	cy 3
	ld hl, $000D
	cy 3
	ASSERT cyc == EXIT_C - TAIL
	jp tail

dmg_mode:
	DEF cyc = 3 + 4 + 2 + 4
	sound
	white_background
	; The default palettes of the DMG's mode: the background's first, both
	; of the objects'.
	ld a, $80
	cy 2
	put $FF68
	colours $FF69, $FF, $7F, $EF, $1B, $80, $61, $00, $00
	ld a, $80
	cy 2
	put $FF6A
	REPT 2
		colours $FF6B, $FF, $7F, $1F, $42, $F2, $1C, $00, $00
	ENDR
	write_at LCD_ON - 1, $FF40, $91
	write_at EXIT_D - DIV_D, $FF04, 0
	wait_to EXIT_D - TAIL - DMG_REGS
	DEF regs_from = cyc
	ld a, $04
	cy 2
	put $FF4C
	ld a, $01
	cy 2
	put $FF6C
	; B is the sum of the title's 16 bytes for Nintendo's games (the old
	; licensee 01, or 33 and the new licensee "01"), 00 otherwise; HL is
	; 991A when that sum is 43 or 58, 007C otherwise. Without branches, so
	; that the time is the same.
	ld hl, $0134
	cy 3
	xor a
	cy 1
	ld c, 16
	cy 2
.sum
	add a, [hl]
	inc hl
	dec c
	jr nz, .sum
	DEF cyc += 8 * 16 - 1
	ld b, a
	cy 1
	ld a, [$014B]
	cy 4
	equal $01
	ld d, a
	cy 1
	ld a, [$014B]
	cy 4
	equal $33
	ld e, a
	cy 1
	ld a, [$0144]
	cy 4
	equal $30  ; "0"
	and e
	cy 1
	ld e, a
	cy 1
	ld a, [$0145]
	cy 4
	equal $31  ; "1"
	and e
	cy 1
	or d
	cy 1
	and b
	cy 1
	ld b, a
	cy 1
	equal $43
	ld d, a
	cy 1
	ld a, b
	cy 1
	equal $58
	or d
	cy 1
	ld d, a
	cy 1
	and $99
	cy 2
	ld h, a
	cy 1
	ld a, d
	cy 1
	and $1A ^ $7C
	cy 2
	xor $7C
	cy 2
	ld l, a
	cy 1
	ld c, $00
	cy 2
	ld de, $0008
	cy 3
	DEF regs_len = cyc - regs_from
	ASSERT regs_len == DMG_REGS, "DMG_REGS should be {d:regs_len}"
	jp tail

	; The boot ROM is 2304 bytes, to 08FF.
	ds $0900 - @, 0
