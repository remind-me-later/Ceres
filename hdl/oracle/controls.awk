# The signals that control storage in the readable model: for each process
# of an rtl/ file, the signals whose edges clock it ("edge") and the ones
# its conditions test before that edge, or in a process with no edge (a
# latch, or logic that a pulse only passes on): "level". Prints a line per
# signal: the entity, the kind and the name (a bit as name(i)), lower case.
#
#   awk -f oracle/controls.awk rtl/*.vhd

FNR == 1 { entity = ""; inproc = 0 }

{
	line = tolower($0)
	sub(/--.*/, "", line)
}

match(line, /^[ \t]*entity[ \t]+[a-z0-9_]+[ \t]+is/) {
	split(substr(line, RSTART, RLENGTH), w, /[ \t]+/)
	entity = (w[1] == "") ? w[3] : w[2]
}

# A process starts (not "end process", not a process's name alone).
!inproc && line ~ /(^|[ \t:])process([ \t(]|$)/ && line !~ /end[ \t]+process/ {
	inproc = 1; clocked = 0; cond = ""; n = 0
}

inproc {
	# The conditions of if and elsif, up to their then.
	text = line
	while (text != "") {
		if (incond) {
			if (match(text, /[ \t]then([ \t]|$)/)) {
				cond = cond " " substr(text, 1, RSTART)
				text = substr(text, RSTART + RLENGTH)
				incond = 0
				conds[++n] = cond
				cond = ""
			} else {
				cond = cond " " text
				text = ""
			}
		} else if (match(text, /(^|[ \t])(els)?if[ \t(]/)) {
			text = substr(text, RSTART + RLENGTH - 1)
			incond = 1
		} else if (match(text, /wait[ \t]+until[ \t]/)) {
			conds[++n] = substr(text, RSTART + RLENGTH)
			text = ""
		} else {
			text = ""
		}
	}
}

inproc && line ~ /end[ \t]+process/ {
	inproc = 0
	# The conditions up to the first edge are level-sensitive; the edges'
	# signals clock the process.
	for (i = 1; i <= n; i++) {
		c = conds[i]
		if (c ~ /_edge[ \t]*\(/) {
			while (match(c, /_edge[ \t]*\([ \t]*[a-z0-9_]+([ \t]*\([0-9a-z_]+\))?/)) {
				s = substr(c, RSTART, RLENGTH)
				sub(/^_edge[ \t]*\([ \t]*/, "", s)
				gsub(/[ \t]/, "", s)
				print entity, "edge", s
				c = substr(c, RSTART + RLENGTH)
			}
			break
		}
		names(c)
	}
	delete conds
}

function names(c,    s) {
	while (match(c, /[a-z][a-z0-9_]*([ \t]*\([0-9]+\))?/)) {
		s = substr(c, RSTART, RLENGTH)
		c = substr(c, RSTART + RLENGTH)
		gsub(/[ \t]/, "", s)
		if (s !~ /^(and|or|not|xor|nand|nor|xnor|then|if|elsif|others|downto|to)$/)
			print entity, "level", s
	}
}
