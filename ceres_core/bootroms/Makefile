.PHONY: all clean

BIN := bin
OBJ := obj
PB12_COMPRESS := $(OBJ)/pb12_compress

all: $(BIN)/cgb_boot.bin $(BIN)/mgb_boot.bin $(BIN)/cgb_boot_fast.bin $(BIN)/dmg_boot.bin

$(OBJ)/%.2bpp: %.png
	mkdir -p $(dir $@)
	rgbgfx -h -u -o $@ $<

$(OBJ)/SameBoyLogo.pb12: $(OBJ)/SameBoyLogo.2bpp $(PB12_COMPRESS)
	$(PB12_COMPRESS) < $< > $@
	
$(PB12_COMPRESS): pb12.c
	cc -std=c99 -Wall -Werror $< -o $@

$(BIN)/cgb_boot.bin: cgb_boot.asm
$(BIN)/cgb0_boot.bin: cgb0_boot.asm
$(BIN)/agb_boot.bin: agb_boot.asm
$(BIN)/cgb_boot_fast.bin: cgb_boot_fast.asm
$(BIN)/sgb2_boot: sgb_boot.asm
$(BIN)/dmg_boot.bin: dmg_boot.asm
$(BIN)/mgb_boot: mgb_boot.asm

$(BIN)/%.bin: %.asm $(OBJ)/SameBoyLogo.pb12
	@mkdir -p $(dir $@)
	rgbasm -i $(OBJ)/ -o $@.tmp $<
	rgblink -o $@.tmp2 $@.tmp
	dd if=$@.tmp2 of=$@ count=1 bs=$(if $(findstring dmg,$@)$(findstring sgb,$@),256,2304)
	@rm $@.tmp $@.tmp2

clean:
	rm -rf build