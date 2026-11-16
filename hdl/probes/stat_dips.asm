; SPDX-License-Identifier: MIT
;
; Whether IF's STAT flag takes the dips of STAT's interrupt line at VBlank's
; end and start (rules.toml, stat-flag-takes-a-flip-flops-dip), in a ROM
; short enough for dmg-sim (see ../README.md):
;
;   printf 'mooneye hdl/build/stat_dips.gb 0.3\n' >build/stat_dips.txt
;   TESTS=build/stat_dips.txt ./run-tests.sh            # dmg-sim
;   MODEL=1 TESTS=build/stat_dips.txt ./run-tests.sh    # the model
;
; D is IF's STAT bit read early in line 0, with STAT's mode 2 and mode 1
; sources on, cleared in line 152: mode 1 falls as the mode 2 source rises,
; and the line dips for two gates (TOLU, TAPA). The DMG-08 does not set the
; flag (Gambatte's m2irq_ly00_lcdstat30): 00. E is the bit read early in
; line 144, with the LY = LYC and mode 1 sources on and LYC 143, cleared in
; line 143: the flag falls a flip-flop before mode 1 rises. The DMG-08 sets
; it (lycint143_m1irq_2): 02. It passes (Mooneye's protocol) with D = 00
; and E = 02, and fails with B = 01, D and E otherwise. With STAT = $20 and
; $40 instead (one source each), both sides give D = 02 and E = 00: the reads
; come after the line's start and before anything else sets the flag.
;
;   rgbasm -o stat_dips.o stat_dips.asm && rgblink -o stat_dips.gb stat_dips.o && rgbfix -v -p 0 stat_dips.gb

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	xor a
	ldh [$FFFF], a            ; IE: nothing, the flag only

	; VBlank's end. A line is 114 M-cycles: line 0 begins 228 M-cycles after
	; line 152 does, and the read comes about 25 M-cycles into it.
	ld a, $30
	ldh [$FF41], a            ; the mode 2 and mode 1 sources
	ld b, 151
	call wait_ly
	ld b, 152
	call wait_ly
	xor a
	ldh [$FF0F], a
	ld hl, 36
	call delay
	ldh a, [$FF0F]
	and 2
	ld d, a

	; VBlank's start: the flag is set early in line 143 and cleared after it.
	ld a, 143
	ldh [$FF45], a            ; LYC
	ld a, $50
	ldh [$FF41], a            ; the LY = LYC and mode 1 sources
	ld b, 142
	call wait_ly
	ld b, 143
	call wait_ly
	ld hl, 4
	call delay
	xor a
	ldh [$FF0F], a
	ld hl, 15
	call delay
	ldh a, [$FF0F]
	and 2
	ld e, a

	; The result: Fibonacci's numbers if D = 00 and E = 02.
	ld a, d
	or a
	jr nz, .fail
	ld a, e
	cp 2
	jr nz, .fail
	ld b, 3
	ld c, 5
	ld d, 8
	ld e, 13
	ld h, 21
	ld l, 34
	jr .done
.fail:
	ld b, 1
.done:
	ld b, b
.halt:
	jr .halt

; Waits for LY = B.
wait_ly:
	ldh a, [$FF44]
	cp b
	jr nz, wait_ly
	ret

; HL times 7 M-cycles.
delay:
	dec hl
	ld a, h
	or l
	jr nz, delay
	ret
