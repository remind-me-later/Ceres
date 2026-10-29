# The short pulses (oracle/runts.awk) that reach a signal controlling
# storage (oracle/controls.awk): the pulses that a flip-flop or a latch may
# take or not, by the die's physics.
#
#   awk -f oracle/runts.awk INSTANCES REPORTS... |
#       awk -f oracle/runts-at-controls.awk CONTROLS -
#
# Prints, for each signal, the kind of control, the number of pulses, the
# shortest one in ns, the entity and signal, and the time of the first
# pulse in ms.

FILENAME == ARGV[1] { control[$1 " " $3] = $2; next }

{
	split($0, f, "\t")
	ent = f[2]
	sig = f[3]
	whole = sig
	sub(/\(.*/, "", whole)
	kind = ""
	if ((ent " " sig) in control) kind = control[ent " " sig]
	else if ((ent " " whole "(i)") in control) kind = control[ent " " whole "(i)"]
	else if ((ent " " whole) in control) kind = control[ent " " whole]
	if (kind == "") next
	key = kind " " ent "." sig
	count[key] += f[5]
	if (!(key in shortest) || f[6] + 0 < shortest[key]) shortest[key] = f[6] + 0
	if (!(key in first) || f[7] + 0 < first[key]) first[key] = f[7] + 0
}

END {
	for (key in count) {
		split(key, k, " ")
		printf "%-5s %8d %7.2f %-40s %.3f\n", k[1], count[key], shortest[key] / 1e6, k[2], first[key] / 1e12
	}
}
