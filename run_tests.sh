cd test
make

cd test_runner

FILES="../bin/*.gb"

for f in $FILES; do
	echo -n "Running $f... "
	cargo run --quiet $f
done

cd ../..
