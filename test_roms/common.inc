; SPDX-License-Identifier: MIT
;
; What the draft test ROMs share: the registers they use, the tiles, and
; macros that count M-cycles, so that a ROM can put a write at a chosen
; M-cycle of a line. A line is 114 M-cycles (456 dots); the line loops of
; these ROMs are exactly that long, so every line sees the same schedule.

DEF rLCDC EQU $FF40
DEF rSTAT EQU $FF41
DEF rSCY  EQU $FF42
DEF rSCX  EQU $FF43
DEF rLY   EQU $FF44
DEF rBGP  EQU $FF47
DEF rOBP0 EQU $FF48
DEF rOBP1 EQU $FF49
DEF rWY   EQU $FF4A
DEF rWX   EQU $FF4B

DEF VRAM_TILES EQU $8000
DEF MAP0       EQU $9800
DEF MAP1       EQU $9C00

; LCDC: LCD on, window map $9C00, window, tile data $8000, background map
; $9800, 8x8 objects, objects, background. The ROMs name the combinations.
DEF LCDC_ON   EQU %10000000
DEF LCDC_WMAP EQU %01000000
DEF LCDC_WIN  EQU %00100000
DEF LCDC_TILE EQU %00010000
DEF LCDC_BMAP EQU %00001000
DEF LCDC_TALL EQU %00000100
DEF LCDC_OBJ  EQU %00000010
DEF LCDC_BG   EQU %00000001

; Waits for VBlank (the LCD may be turned off there).
MACRO wait_vblank
.wait\@:
	ldh a, [rLY]
	cp 144
	jr c, .wait\@
ENDM

; Eight rows of a tile, each one colour (0 to 3); two bytes a row.
MACRO tile_rows
	REPT 8
		db (\1 & 1) * $FF, (\1 >> 1) * $FF
		SHIFT
	ENDR
ENDM

; A tile of one colour.
MACRO tile_solid
	tile_rows \1, \1, \1, \1, \1, \1, \1, \1
ENDM

; Copies BC bytes from HL to DE.
MACRO copy_bc
.copy\@:
	ld a, [hl+]
	ld [de], a
	inc de
	dec bc
	ld a, b
	or c
	jr nz, .copy\@
ENDM

; Fills BC bytes at HL with A (clobbers D).
MACRO fill_bc
.fill\@:
	ld [hl+], a
	dec bc
	ld d, a
	ld a, b
	or c
	ld a, d
	jr nz, .fill\@
ENDM

; ---------------------------------------------------------------------
; Cycle counting. `cyc` is the M-cycle of the line the next instruction
; starts in, from 0 at the top of the line loop. The macros emit the
; instruction and move `cyc`; `m_write_at` and `m_at` pad with nops so that
; the write lands in a given M-cycle, and fail to assemble if it is too late.
; A write instruction (LDH [n], A) writes in its last (third) M-cycle.
; ---------------------------------------------------------------------

DEF cyc = 0

MACRO m_nop
	REPT \1
		nop
	ENDR
	DEF cyc += \1
ENDM

; LD A, n: 2 M-cycles.
MACRO m_ld_a
	ld a, \1
	DEF cyc += 2
ENDM

; LDH A, [n]: 3 M-cycles, reads in the last.
MACRO m_read
	ldh a, [\1]
	DEF cyc += 3
ENDM

; Waits until the next instruction starts in M-cycle \1.
MACRO m_pad_to
	IF (\1) < cyc
		FAIL "m_pad_to: too late"
	ENDC
	m_nop (\1) - cyc
ENDM

; Writes A to the register \2 in M-cycle \1.
MACRO m_write_a_at
	m_pad_to (\1) - 2
	ldh [\2], a
	DEF cyc += 3
ENDM

; Writes the value \3 to the register \2 in M-cycle \1.
MACRO m_write_at
	m_pad_to (\1) - 4
	m_ld_a \3
	ldh [\2], a
	DEF cyc += 3
ENDM

; Writes the register \3 (B, C, D, E, H or L) to the register \2 in M-cycle
; \1: for writes that are too close together to load A with a number.
MACRO m_write_r_at
	m_pad_to (\1) - 3
	ld a, \3
	DEF cyc += 1
	ldh [\2], a
	DEF cyc += 3
ENDM

; Writes the register \2 (B, C, D, E) to the address in HL in M-cycle \1,
; with LD [HL], r: 2 M-cycles, so that two writes can be 2 M-cycles apart.
MACRO m_write_hl_at
	m_pad_to (\1) - 1
	ld [hl], \2
	DEF cyc += 2
ENDM

; The end of a line: pads to 110 and jumps to the label \1 (JP, 4 M-cycles),
; so that the loop is 114 M-cycles long.
MACRO m_line_end
	m_pad_to 110
	jp \1
	DEF cyc += 4
ENDM
