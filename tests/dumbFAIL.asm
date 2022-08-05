INCLUDE "gbhw.inc"

SECTION "start",ROM0[$0100]
	nop
	jp inicio

	ROM_HEADER  ROM_NOMBC, ROM_SIZE_32KBYTE, RAM_SIZE_0KBYTE

inicio:
	di
	ld b,3
	ld c,5
	ld d,8
	ld e,13
	ld h,21
	ld l,17 ; non-zero means FAIL
.do_nothing:
	nop
	jr .do_nothing

