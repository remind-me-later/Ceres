cd ceres_test_runner
FILES="../tests/*"
for f in $FILES
do
	cp $f test.asm
	rgbasm test.asm -o obj.o
	rgblink obj.o -o obj.gb
	rgbfix -v -p 0 obj.gb
	echo -n "Running $f... "
	cargo run --quiet obj.gb
done
rm test.asm
rm obj.o
rm obj.gb
cd ..
