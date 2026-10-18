; SPDX-License-Identifier: MIT
;
; A stimulus for the CPU core's check (see ../README.md): it executes every
; opcode but STOP and the eleven that lock the CPU up, and all those after
; the CB prefix, with operands that set and clear the flags; the conditional
; jumps, calls and returns both ways; DAA after additions and subtractions;
; and interrupts: their priority, EI, DI, RETI and HALT, with the timer
; and with writes to IF. It reports no result: dmg-sim runs it, and the
; readable model is checked against the signals it records.
;
;   rgbasm -o cpu.o cpu.asm && rgblink -o cpu.gb cpu.o && rgbfix -v -p 0 cpu.gb

DEF RAM EQU $C000   ; the work area, which HL points into

; Points HL into the work area, at RAM + \1.
MACRO hl_ram
	ld hl, RAM + (\1)
ENDM

; The RST vectors and the interrupts' vectors. An interrupt's handler counts
; it in its own byte of high RAM (FFA0 to FFA4: not FF80 to FF82, where
; gbmicrotest's ROMs report their result).
SECTION "rst00", ROM0[$0000]
	ret
SECTION "rst08", ROM0[$0008]
	ret
SECTION "rst10", ROM0[$0010]
	ret
SECTION "rst18", ROM0[$0018]
	ret
SECTION "rst20", ROM0[$0020]
	ret
SECTION "rst28", ROM0[$0028]
	ret
SECTION "rst30", ROM0[$0030]
	ret
SECTION "rst38", ROM0[$0038]
	ret

MACRO handler
SECTION "int\1", ROM0[$0040 + 8 * \1]
	push af
	ldh a, [$FFA0 + \1]
	inc a
	ldh [$FFA0 + \1], a
	pop af
	reti
ENDM
	handler 0
	handler 1
	handler 2
	handler 3
	handler 4

SECTION "entry", ROM0[$0100]
	nop
	jp main

SECTION "main", ROM0[$0150]
main:
	di
	ld sp, $DFFF
	xor a
	ldh [$FF0F], a
	ldh [$FFFF], a

	; Registers with mixed bits.
	ld bc, $81C3
	ld de, $7E0F
	hl_ram $80
	ld a, $5A
	and a

	; 8-bit loads LD r,r' (40 to 7F but HALT), with HL in the work area for
	; the ones with (HL).
FOR OP, $40, $80
IF OP != $76
	hl_ram $40 + (OP & 7)
	db OP
ENDC
ENDR

	; The ALU on A (80 to BF), from two values of A, with the flags set by
	; the one before.
FOR OP, $80, $C0
	hl_ram $40 + (OP & 7)
	ld a, $3C + OP
	db OP
	ld a, $C5
	db OP
ENDR

	; The ALU with an immediate (C6 to FE).
FOR OP, $C6, $100, 8
	ld a, $3C
	db OP, $C4
	ld a, $F0
	db OP, $0F
ENDR

	; INC r and DEC r (04, 05, 0C, ...), INC (HL) and DEC (HL), at 00, 0F,
	; 7F and FF.
FOR V, 0, 4
	ld b, LOW($0F * V + (V == 3) * $D2)
	ld c, b
	ld d, b
	ld e, b
	ld a, b
	hl_ram $10
	ld [hl], b
FOR OP, $04, $40, 8
	IF OP != $34 && OP != $35
		IF (OP >> 3) == 4 || (OP >> 3) == 5
			; H and L: keep HL in the work area
			push hl
			db OP, OP + 1
			pop hl
		ELSE
			db OP, OP + 1, OP
		ENDC
	ENDC
ENDR
	inc [hl]
	dec [hl]
	dec [hl]
ENDR

	; The rotations of A, DAA, CPL, SCF, CCF.
	ld a, $96
	rlca
	rrca
	rla
	rra
	cpl
	scf
	ccf
	ccf

	; DAA after additions and subtractions, with and without carries.
	ld b, 0
.daa:
	ld a, b
	add a, $19
	daa
	ld a, b
	adc a, $99
	daa
	ld a, b
	sub a, $27
	daa
	ld a, b
	sbc a, $08
	daa
	ld a, b
	add a, $13
	ld b, a
	jr nc, .daa

	; 16-bit: LD rr,nn, INC rr, DEC rr, ADD HL,rr (SP put back after).
	ld bc, $0FFF
	ld de, $FFFF
	ld hl, $8F80
	inc bc
	inc de
	inc hl
	dec bc
	dec de
	dec hl
	add hl, bc
	add hl, de
	add hl, hl
	ld hl, $0001
	add hl, sp
	ld sp, $DF00
	inc sp
	dec sp
	dec sp
	ld sp, $DFFF

	; ADD SP,e, LD HL,SP+e, LD SP,HL, LD (nn),SP.
	add sp, -2
	add sp, 5
	add sp, -3
	ld hl, sp + 7
	ld hl, sp - 128
	ld hl, sp + 127
	ld [RAM + $20], sp
	ld hl, $DFF0
	ld sp, hl
	ld sp, $DFFF

	; Loads of A through BC, DE, HL+ and HL-; LD r,n and LD (HL),n.
	ld bc, RAM + $30
	ld de, RAM + $31
	hl_ram $32
	ld a, $A7
	ld [bc], a
	ld [de], a
	ld [hl+], a
	ld [hl-], a
	ld a, [bc]
	ld a, [de]
	ld a, [hl+]
	ld a, [hl-]
	ld b, $11
	ld c, $22
	ld d, $33
	ld e, $44
	ld h, HIGH(RAM)
	ld l, $34
	ld [hl], $55
	ld a, $66

	; LDH and LD (nn).
	ldh [$FF90], a
	ldh a, [$FF90]
	ld c, $91
	ldh [c], a
	ldh a, [c]
	ld [RAM + $38], a
	ld a, [RAM + $38]

	; PUSH and POP, AF with all flags.
	ld bc, $1234
	ld de, $5678
	ld hl, $9ABC
	push bc
	push de
	push hl
	push af
	pop bc
	pop de
	pop hl
	pop af
	ld bc, $FFFF
	push bc
	pop af
	ld bc, $0000
	push bc
	pop af
	push af
	pop de

	; After the CB prefix: all 256, on registers with mixed bits and (HL).
FOR OP, 0, 256
	ld bc, $81C3 ^ (OP * $0101)
	ld de, $7E0F ^ OP
	ld a, $5A ^ OP
	hl_ram $50
	ld [hl], b
	db $CB, OP
ENDR

	; Jumps: JR, JP, JP HL, each condition taken and not.
	xor a        ; Z, NC
	jr nz, @+2
	jr z, @+2
	jr c, @+2
	jr nc, @+2
	scf
	jr c, @+2
	jr nc, @+2
	jr @+2
	jp nz, .j1
.j1:
	jp z, .j2
.j2:
	jp c, .j3
.j3:
	or 1         ; NZ, NC
	jp nc, .j4
.j4:
	jp .j5
.j5:
	ld hl, .j6
	jp hl
.j6:
	jr .jr_back_test
.jr_back:
	jr .jr_back_done
.jr_back_test:
	jr .jr_back   ; backwards
.jr_back_done:

	; Calls and returns, each condition taken and not; RST.
	xor a
	call .sub
	call nz, .sub
	call z, .sub
	call c, .sub
	call nc, .sub
	scf
	call c, .sub_cc
	call nc, .sub_cc
	rst $00
	rst $08
	rst $10
	rst $18
	rst $20
	rst $28
	rst $30
	rst $38
	jr .calls_done
.sub:
	ret
.sub_cc:
	ret nc
	ret z
	ret nz
.calls_done:

	; Interrupts: all five requested at once in IF, taken in their order.
	ld a, $1F
	ldh [$FFFF], a
	ldh [$FF0F], a
	ei
	nop
	nop
	di
	; EI then DI at once: none is taken.
	ld a, $1F
	ldh [$FF0F], a
	ei
	di
	nop
	; RETI as the return of a CALL (it sets IME).
	call .reti
	nop
	di
	; HALT until the timer's interrupt, with IME set, then with IME clear
	; (HALT ends without the interrupt), then with an interrupt already
	; requested (the HALT bug).
	xor a
	ldh [$FF0F], a
	ld a, $04
	ldh [$FFFF], a
	ld a, $F0
	ldh [$FF06], a   ; TMA: an interrupt every 64 M-cycles
	ldh [$FF05], a   ; TIMA
	ld a, $05
	ldh [$FF07], a   ; TAC: on, 262 kHz
	ei
	halt
	nop
	di
	halt
	nop
	ld a, $04
	ldh [$FF0F], a
	halt
	inc a
	; The timer's interrupt in the middle of a loop.
	ei
	ld b, 0
.loop:
	inc b
	jr nz, .loop
	di
	xor a
	ldh [$FF07], a
	ldh [$FFFF], a

.end:
	jr .end

.reti:
	reti
