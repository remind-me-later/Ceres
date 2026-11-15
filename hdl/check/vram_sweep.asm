; SPDX-License-Identifier: MIT
;
; CPU accesses to VRAM and OAM swept across the end of mode 3, for the
; gate tallies (ceres-netlist's and ceres-test-runner's tests/vram_gate.rs
; and oam_gate.rs), which judge each access by mode 3's level in its
; phases. Their ROMs reach only some of the alignments of an access with
; mode 3's fall; here every one is reached: SCX 0 to 3 moves mode 3's end
; a dot (2 half dots) at a time, through the four alignments of an
; M-cycle's half dots it can fall on, and a NOP sled moves the access an
; M-cycle at a time, from 3 before the end (with SCX 0) on.
;
; Every other visible line does one access: the CPU halts until mode 2 starts
; (STAT's mode 2 interrupt, IME off), waits DELAY_MIN + d M-cycles through
; the sled, and reads or writes one byte, of VRAM (8010) or of OAM
; (sprite 39's tile, FE9E; sprite 39 is off the screen). The four kinds of
; access, SCX and d (0 to DELAYS - 1) are swept in that order, twice: with
; no sprites, then with tall sprites at X 167 on every line, whose fetch
; holds mode 3 past a pulse of mode 0 (sprite-at-167-mode-0-pulse). A
; write writes a byte that changes from line to line, so that the tallies
; can tell whether it landed (the bytes are never FF), and reads it back
; in HBlank, so that the lockstep oracle (ceres-test-runner's example
; lockstep, which compares each access's byte with NetChip's) sees it too. It checks nothing
; itself: it runs for 0.29 s, as the tallies do.
;
;   make build/vram_sweep.gb

DEF VRAM_BYTE EQU $8010
DEF OAM_BYTE EQU $FE9E
DEF DELAY_MIN EQU 51
DEF DELAYS EQU 10

; HRAM: the byte the writes write, the kind of access, SCX.
DEF VALUE EQU $FF80
DEF KIND EQU $FF81
DEF SCX EQU $FF82

SECTION "entry", ROM0[$0100]
	nop
	jp Main

SECTION "main", ROM0[$0150]
Main:
	di
	ld sp, $FFFE
	ld a, $97
	ldh [$FF40], a              ; LCD, tall objects, objects, background on
	call WaitVblank
	ld a, $5A
	ld [VRAM_BYTE], a
	; Every sprite off the screen, sprite 39's tile 5A.
	ld hl, $FE00
	ld b, 160
	xor a
.clear:
	ld [hl+], a
	dec b
	jr nz, .clear
	ld a, $5A
	ld [OAM_BYTE], a
	ld a, $11
	ldh [VALUE], a
	xor a
	ldh [$FF0F], a
	ld a, $02
	ldh [$FFFF], a              ; IE: STAT
	ld a, $20
	ldh [$FF41], a              ; STAT: the mode 2 interrupt

	call Sweep
	; Nine tall sprites at X 167, one on every line.
	call WaitVblank
	ld hl, $FE00
	ld a, 16
	ld b, 9
.sprites:
	ld [hl+], a                 ; Y
	ld [hl], 167                ; X
	inc hl
	inc hl
	inc hl
	add 16
	dec b
	jr nz, .sprites
	call Sweep
.done:
	jr .done

; The four kinds, SCX 0 to 3, the DELAYS delays: an access each (every
; other line: the loop misses the next line's mode 2).
Sweep:
	xor a
	ldh [KIND], a
.kind:
	xor a
	ldh [SCX], a
.scx:
	ld b, 0
.delay:
	call Line
	inc b
	ld a, b
	cp DELAYS
	jr nz, .delay
	ldh a, [SCX]
	inc a
	ldh [SCX], a
	cp 4
	jr nz, .scx
	ldh a, [KIND]
	inc a
	ldh [KIND], a
	cp 4
	jr nz, .kind
	ret

; A line: halt until mode 2, SCX, then the sled from DELAYS - B NOPs before
; its end, and the access.
Line:
	; The value written changes each line, and is never FF.
	ldh a, [VALUE]
	add $25
	cp $FF
	jr nz, .value
	inc a
.value:
	ldh [VALUE], a
	ld d, a
	; The sled's entry: B NOPs from its end, so a longer B waits less.
	ldh a, [KIND]
	add a
	add LOW(Sleds)
	ld l, a
	ld a, 0
	adc HIGH(Sleds)
	ld h, a
	ld a, [hl+]
	ld h, [hl]
	ld l, a                     ; HL: the kind's sled end
	ld a, l
	sub b
	ld l, a
	jr nc, .entry
	dec h
.entry:
	ldh a, [SCX]
	ld e, a
	xor a
	ldh [$FF0F], a
	halt                        ; until mode 2 starts (IME off: no call)
	ld a, e
	ldh [$FF43], a              ; SCX, before this line's mode 3
	ld a, d
	jp hl

Sleds:
	dw ReadVram, WriteVram, ReadOam, WriteOam

; Each kind: DELAYS NOPs, of which the entry skips all but B, then
; DELAY_MIN NOPs always, the access, and back.
	REPT DELAYS
		nop
	ENDR
ReadVram:
	REPT DELAY_MIN
		nop
	ENDR
	ld a, [VRAM_BYTE]
	ret
	REPT DELAYS
		nop
	ENDR
WriteVram:
	REPT DELAY_MIN
		nop
	ENDR
	ld [VRAM_BYTE], a
	ld a, [VRAM_BYTE]           ; read back in HBlank: did it land?
	ret
	REPT DELAYS
		nop
	ENDR
ReadOam:
	REPT DELAY_MIN
		nop
	ENDR
	ld a, [OAM_BYTE]
	ret
	REPT DELAYS
		nop
	ENDR
WriteOam:
	REPT DELAY_MIN
		nop
	ENDR
	ld [OAM_BYTE], a
	ld a, [OAM_BYTE]            ; read back in HBlank: did it land?
	ret

WaitVblank:
	ldh a, [$FF44]
	cp 145
	jr nz, WaitVblank
	ret
