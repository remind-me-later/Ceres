# The short pulses that oracle/runts.c reports, with the entity of the
# instance each signal is in, as tab-separated lines: the ROM (the report's
# path under reports/), the entity, the signal (a bit as name(i), lower
# case), the path, the number of pulses, the shortest one and the first
# one's time (in femtoseconds) and their level (0, 1, b).
#
#   awk -f oracle/runts.awk INSTANCES REPORTS...
#
# INSTANCES: lines "entity label entity-of-the-instance" (knowledge/rtl.awk,
# TABLE=instance), tab-separated.

FILENAME == ARGV[1] { split($0, f, "\t"); entity[f[1] " " f[2]] = f[3]; next }

# The ROM: the report's path under reports/.
FNR == 1 {
	rom = FILENAME
	sub(/^(.*\/)?reports\//, "", rom)
	sub(/\.txt$/, "", rom)
}

{
	path = tolower($5)
	n = split(path, part, ":")
	# The entity of the innermost instance, from the top (a generate's
	# label is not one).
	ent = part[2]
	for (i = 3; i < n; i++) {
		label = part[i]
		sub(/\(.*/, "", label)
		if ((ent " " label) in entity) ent = entity[ent " " label]
	}
	print rom "\t" ent "\t" part[n] "\t" path "\t" $1 "\t" $2 "\t" $3 "\t" $4
}
