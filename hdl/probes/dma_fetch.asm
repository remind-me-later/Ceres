; SPDX-License-Identifier: MIT
;
; What the sprite fetch of mode 3 reads while an OAM DMA runs: on the
; netlist, OAM's latches take the data lines, which carry the DMA's byte,
; not OAM. The DMA copies C100 to C19F, where byte n is n + 16: as OAM,
; that puts two sprites on every line, at X spread over the screen, and the
; value OAM's first latches take names the DMA byte. The DMAs follow each
; other with a delay that grows by an M-cycle each time, so that the
; fetches fall at every half dot of the DMA's M-cycles. Trace it on the
; model (../trace-blocks.sh), and see the first latches (`first_even`) as
; `fetch_read` falls while `run` is on, against the DMA's count:
;
;   ./trace-blocks.sh hdl/build/dma_fetch.gb 0.1
;
;   rgbasm -o dma_fetch.o dma_fetch.asm && rgblink -o dma_fetch.gb dma_fetch.o && rgbfix -v -p 0 dma_fetch.gb

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	ld sp, $FFFE
	; The DMA's source: byte n is n + 16.
	ld hl, $C100
	ld a, 16
.fill:
	ld [hl+], a
	inc a
	cp 16 + 160
	jr nz, .fill
	; The DMA routine, in HRAM: the CPU runs there while the DMA has the bus.
	ld hl, dma_routine
	ld de, $FF80
	ld b, dma_routine_end - dma_routine
.copy:
	ld a, [hl+]
	ld [de], a
	inc e
	dec b
	jr nz, .copy
	ld a, $93
	ldh [$FF40], a              ; LCD, background and sprites on
	ld c, 0
.loop:
	call $FF80
	; A pause in which the scan reads OAM again, ...
	ld b, 25
.pause:
	dec b
	jr nz, .pause
	; ... and c more M-cycles, 0 to 15: a jump into a run of NOPs.
	ld a, c
	inc a
	and 15
	ld c, a
	ld hl, .sled_end
	ld a, l
	sub c
	ld l, a
	ld a, h
	sbc 0
	ld h, a
	jp hl
.sled:
	REPT 16
	nop
	ENDR
.sled_end:
	jr .loop

; Starts the DMA and waits the 160 M-cycles of its run.
dma_routine:
	ld a, $C1
	ldh [$FF46], a
	ld a, 40
.wait:
	dec a
	jr nz, .wait
	ret
dma_routine_end:
