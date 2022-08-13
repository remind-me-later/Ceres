.PHONY: all clean

PB12_COMPRESS := obj/pb12_compress

all: bin/cgb_boot.bin bin/mgb_boot.bin bin/cgb_boot_fast.bin bin/dmg_boot.bin

obj/%.2bpp: %.png
	mkdir -p $(dir $@)
	rgbgfx -h -u -o $@ $<

obj/SameBoyLogo.pb12: obj/SameBoyLogo.2bpp $(PB12_COMPRESS)
	$(PB12_COMPRESS) < $< > $@
	
$(PB12_COMPRESS): pb12.c
	cc -std=c99 -Wall -Werror $< -o $@
 
bin/%.bin: %.asm obj/SameBoyLogo.pb12
	@mkdir -p $(dir $@)
	rgbasm -i obj/ -o $@.tmp $<
	rgblink -o $@.tmp2 $@.tmp
	dd if=$@.tmp2 of=$@ count=1 bs=$(if $(findstring dmg,$@)$(findstring sgb,$@),256,2304)
	@rm $@.tmp $@.tmp2

clean:
	rm -rf bin obj