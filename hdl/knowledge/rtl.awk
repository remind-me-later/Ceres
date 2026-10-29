# Tables of the readable model's sources, for the knowledge database
# (../knowledge.sh), as tab-separated lines:
#
#   awk -v TABLE=instance -f knowledge/rtl.awk rtl/*.vhd board/*.vhd
#       the entity, an instance's label in it, the instance's entity
#   awk -v TABLE=delay -f knowledge/rtl.awk rtl/*.vhd
#       the file, the line, the entity, the signal assigned and its delays
#       (the `after` clauses, apart by spaces; tco is a flip-flop's)
#   awk -v TABLE=signal -f knowledge/rtl.awk rtl/*.vhd
#       the entity, and a signal or port declared in it
#   awk -v TABLE=hw -f knowledge/rtl.awk rtl/*.vhd
#       the file, the line, the entity, the kind of `@hw` tag (die,
#       measured, inferred) and what it says
#   awk -v TABLE=rule -f knowledge/rtl.awk rtl/*.vhd
#       a rule of rules.toml that an `@rule` tag names, the file and the
#       line
#
# The file names are relative to the repository (they are given relative
# to hdl/).

FNR == 1 { entity = ""; file = "hdl/" FILENAME; stmt = ""; first = 0 }

{
	raw = $0
	line = tolower($0)
	comment = ""
	if (match(line, /--.*/)) {
		comment = substr(raw, RSTART + 2)
		line = substr(line, 1, RSTART - 1)
	}
}

match(line, /^entity[ \t]+[a-z0-9_]+/) {
	split(line, w, /[ \t]+/)
	entity = w[2]
}

TABLE == "instance" && match(line, /^[ \t]*[a-z0-9_]+[ \t]*:[ \t]*entity[ \t]+work\.[a-z0-9_]+/) {
	s = substr(line, RSTART, RLENGTH)
	gsub(/[ \t]/, "", s)
	split(s, w, /:entitywork\./)
	print entity "\t" w[1] "\t" w[2]
}

# A signal's or a port's declaration: its names, apart by commas.
TABLE == "signal" && (line ~ /^[ \t]*signal[ \t]/ ||
                      line ~ /^[ \t]*[a-z0-9_, \t]+:[ \t]*(in|out|inout|buffer)[ \t]/) {
	s = line
	sub(/^[ \t]*signal[ \t]+/, "", s)
	sub(/^[ \t]*port[ \t]*\([ \t]*/, "", s)
	sub(/:.*/, "", s)
	n = split(s, names, /[ \t]*,[ \t]*/)
	for (i = 1; i <= n; i++) {
		gsub(/[ \t]/, "", names[i])
		if (names[i] != "")
			print entity "\t" names[i]
	}
}

TABLE == "rule" {
	s = comment
	while (match(s, /@rule[ \t]+[a-z0-9-]+/)) {
		id = substr(s, RSTART, RLENGTH)
		sub(/@rule[ \t]+/, "", id)
		print id "\t" file "\t" FNR
		s = substr(s, RSTART + RLENGTH)
	}
}

TABLE == "hw" && match(comment, /@hw[ \t]+(die|measured|inferred):?/) {
	kind = substr(comment, RSTART, RLENGTH)
	sub(/@hw[ \t]+/, "", kind)
	sub(/:$/, "", kind)
	text = substr(comment, RSTART + RLENGTH)
	sub(/^[ \t]+/, "", text)
	print file "\t" FNR "\t" entity "\t" kind "\t" text
}

# A concurrent or sequential assignment with delays, over one or more
# lines, up to its semicolon.
TABLE == "delay" {
	if (stmt == "" && line ~ /<=/) {
		stmt = line
		first = FNR
	} else if (stmt != "") {
		stmt = stmt " " line
	}
	if (stmt != "" && line ~ /;/) {
		if (stmt ~ /[ \t]after[ \t]/) {
			target = stmt
			sub(/[ \t]*<=.*/, "", target)
			gsub(/\([^)]*\)/, "", target)
			sub(/^.*[: \t]/, "", target)
			delays = ""
			s = stmt
			while (match(s, /[ \t]after[ \t]+[a-z0-9_]+([ \t]+(ns|ps))?/)) {
				d = substr(s, RSTART, RLENGTH)
				sub(/^[ \t]after[ \t]+/, "", d)
				gsub(/[ \t]+/, "", d)
				if (index(" " delays " ", " " d " ") == 0)
					delays = delays (delays == "" ? "" : " ") d
				s = substr(s, RSTART + RLENGTH)
			}
			print file "\t" first "\t" entity "\t" target "\t" delays
		}
		stmt = ""
	}
}
