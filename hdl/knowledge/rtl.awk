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
#       a rule of rules.toml that an `@rule` tag names, the file, the line,
#       the entity, and the chips the code it is in is for: all, or the
#       DMG's (dmg) or the CGB's (cgb) of an `if REVISION = DMG_B generate`
#       or `if REVISION /= DMG_B generate` and its `else generate`
#   awk -v TABLE=port -f knowledge/rtl.awk rtl/*.vhd
#       the entity, a port of it and its direction (in, out, inout, buffer)
#   awk -v TABLE=portmap -f knowledge/rtl.awk rtl/*.vhd
#       the entity, an instance's label in it, the instance's entity, a
#       port of the instance and a signal of the entity in its actual
#   awk -v TABLE=flow -f knowledge/rtl.awk rtl/*.vhd
#       the entity, a signal (or variable) read in an assignment, and the
#       one assigned: in the expression, in the conditions of the ifs and
#       cases around it, in a `with ... select`
#   awk -v TABLE=anchor -f knowledge/rtl.awk rtl/*.vhd
#       the kind of tag (fact or rule), its id, the entity, and a signal
#       that the statement after the tag assigns (all of a process's, if a
#       process follows), for the tags in an architecture
#
# The file names are relative to the repository (they are given relative
# to hdl/). The flow is a reading of the text, not an elaboration: an
# entity's instances share its signals, and a signal read anywhere in an
# assignment counts.

FNR == 1 {
	entity = ""; file = "hdl/" FILENAME; stmt = ""; first = 0; depth = 0; branch[0] = "all"
	inarch = 0; ifs = 0; flowstmt = ""; withsel = ""; inst = ""; nanchors = 0; inproc = 0; procanchors = 0
	inports = 0
}

# The names an entity declares (ports, signals, variables), the only ones
# the flow counts.
function signals_in(text,    n, toks, i, out, seen) {
	n = split(text, toks, /[^a-z0-9_]+/)
	out = ""
	for (i = 1; i <= n; i++)
		if (toks[i] != "" && ((file SUBSEP toks[i]) in declared) && !(toks[i] in seen)) {
			seen[toks[i]] = 1
			out = out " " toks[i]
		}
	return out
}

function conditions(    i, out) {
	out = ""
	for (i = 1; i <= ifs; i++)
		out = out " " cond[i]
	return out
}

{
	raw = $0
	line = tolower($0)
	comment = ""
	if (match(line, /--.*/)) {
		comment = substr(raw, RSTART + 2)
		line = substr(line, 1, RSTART - 1)
	}
}

# The generate statements the line is in, and the chips each is for: an
# opening pushes its branch (another generate keeps the one it is in), an
# else swaps the DMG's and the CGB's, an end pops.
line ~ /:[ \t]*if[ \t]+revision[ \t]*=[ \t]*dmg_b[ \t]+generate/ {
	branch[++depth] = "dmg"
}
line ~ /:[ \t]*if[ \t]+revision[ \t]*\/=[ \t]*dmg_b[ \t]+generate/ {
	branch[++depth] = "cgb"
}
line ~ /:[ \t]*(if|for)[ \t].*generate/ && line !~ /revision[ \t]*\/?=[ \t]*dmg_b/ {
	branch[depth + 1] = branch[depth]
	depth++
}
line ~ /^[ \t]*else[ \t]+generate/ {
	if (branch[depth] == "dmg")
		branch[depth] = "cgb"
	else if (branch[depth] == "cgb")
		branch[depth] = "dmg"
}
line ~ /^[ \t]*end[ \t]+generate/ && depth > 0 {
	depth--
}

match(line, /^entity[ \t]+[a-z0-9_]+/) {
	split(line, w, /[ \t]+/)
	entity = w[2]
}

line ~ /^architecture[ \t]/ { inarch = 1 }

# Declarations: the names, and a port's direction.
{
	decl = ""
	if (line ~ /^[ \t]*(signal|variable|alias)[ \t]/) {
		decl = line
		sub(/^[ \t]*(signal|variable|alias)[ \t]+/, "", decl)
		sub(/:.*/, "", decl)
	} else if (!inarch && line ~ /^[ \t]*[a-z0-9_, \t]+:[ \t]*(in|out|inout|buffer)[ \t]/) {
		decl = line
		sub(/^[ \t]*port[ \t]*\([ \t]*/, "", decl)
		dir = line
		sub(/^[^:]*:[ \t]*/, "", dir)
		sub(/[ \t].*/, "", dir)
		sub(/:.*/, "", decl)
	}
	if (decl != "") {
		nd = split(decl, dn, /[ \t]*,[ \t]*/)
		for (i = 1; i <= nd; i++) {
			gsub(/[ \t]/, "", dn[i])
			if (dn[i] == "")
				continue
			declared[file, dn[i]] = 1
			if (TABLE == "port" && dir != "")
				print entity "\t" dn[i] "\t" dir
		}
		dir = ""
	}
}

# An instance's port map: each formal, and the entity's signals in its
# actual (`others` is an aggregate's, not a port).
TABLE == "portmap" && match(line, /^[ \t]*[a-z0-9_]+[ \t]*:[ \t]*entity[ \t]+work\.[a-z0-9_]+/) {
	s = substr(line, RSTART, RLENGTH)
	gsub(/[ \t]/, "", s)
	split(s, w, /:entitywork\./)
	inst = w[1]; child = w[2]; mapped = ""
}
TABLE == "portmap" && inst != "" {
	mapped = mapped " " line
	if (mapped ~ /port[ \t]+map/ && line ~ /\)[ \t]*;/) {
		m = mapped
		sub(/.*port[ \t]+map[ \t]*\(/, "", m)
		nf = 0
		while (match(m, /[a-z0-9_]+[ \t]*(\([^)]*\))?[ \t]*=>/)) {
			f = substr(m, RSTART, RLENGTH)
			sub(/[ \t]*(\([^)]*\))?[ \t]*=>$/, "", f)
			if (nf > 0)
				actual[nf] = substr(m, 1, RSTART - 1)
			formal[++nf] = f
			m = substr(m, RSTART + RLENGTH)
		}
		actual[nf] = m
		for (k = 1; k <= nf; k++) {
			if (formal[k] == "others")
				continue
			ns = split(signals_in(actual[k]), sg, " ")
			for (j = 1; j <= ns; j++)
				print entity "\t" inst "\t" child "\t" formal[k] "\t" sg[j]
		}
		inst = ""
	}
}

# The flow: the conditions around an assignment (ifs and cases in
# processes, not generates), and its sources.
(TABLE == "flow" || TABLE == "anchor") && inarch {
	code = line
	if (code ~ /^[ \t]*(if|elsif)[ \t]/ && code !~ /generate/) {
		c = code
		sub(/^[ \t]*(els)?if[ \t]+/, "", c)
		sub(/[ \t]then.*/, "", c)
		if (code ~ /^[ \t]*if[ \t]/)
			ifs++
		cond[ifs] = signals_in(c)
		# An assignment after `then` on the same line.
		code = (code ~ /[ \t]then[ \t]/) ? substr(code, index(code, " then ") + 6) : ""
	} else if (code ~ /^[ \t]*case[ \t].*[ \t]is/) {
		c = code
		sub(/^[ \t]*case[ \t]+/, "", c)
		sub(/[ \t]is.*/, "", c)
		cond[++ifs] = signals_in(c)
		code = ""
	} else if (code ~ /^[ \t]*end[ \t]+(if|case)/) {
		if (ifs > 0)
			ifs--
		code = ""
	} else if (code ~ /^[ \t]*when[ \t].*=>/ && flowstmt == "") {
		sub(/^[ \t]*when[ \t].*=>/, "", code)
	} else if (code ~ /^[ \t]*(signal|variable|constant|alias|type|subtype|function|procedure)[ \t]/) {
		code = ""
	}
	# A tag on an instance anchors nothing.
	if (code ~ /:[ \t]*entity[ \t]+work\./)
		nanchors = 0
	if (code ~ /process/ && code !~ /end[ \t]+process/) {
		inproc = 1
		procanchors = nanchors
		ifs = 0
	}
	if (code ~ /end[ \t]+process/) {
		inproc = 0; procanchors = 0; nanchors = 0; ifs = 0
	}
	if (code ~ /^[ \t]*with[ \t].*[ \t]select/) {
		withsel = code
		sub(/^[ \t]*with[ \t]+/, "", withsel)
		sub(/[ \t]select.*/, "", withsel)
		withsel = signals_in(withsel)
		sub(/.*[ \t]select[ \t]*/, "", code)
	}
	if (flowstmt == "" && code ~ /(<=|:=)/)
		flowstmt = code
	else if (flowstmt != "")
		flowstmt = flowstmt " " code
	if (flowstmt != "" && code ~ /;/) {
		target = flowstmt
		sub(/[ \t]*(<=|:=).*/, "", target)
		gsub(/\([^)]*\)/, "", target)
		sub(/^.*[: \t]/, "", target)
		rhs = flowstmt
		sub(/^[^<:]*(<=|:=)/, "", rhs)
		if ((file SUBSEP target) in declared) {
			ns = split(signals_in(rhs " " conditions() " " withsel), sg, " ")
			if (TABLE == "flow")
				for (j = 1; j <= ns; j++)
					print entity "\t" sg[j] "\t" target
			if (TABLE == "anchor") {
				for (j = 1; j <= nanchors; j++)
					print akind[j] "\t" aid[j] "\t" entity "\t" target
				if (!inproc)
					nanchors = 0
			}
		}
		flowstmt = ""
		withsel = ""
	}
}

# The tags in an architecture, which the next statement (or process)
# anchors.
TABLE == "anchor" && inarch && comment ~ /@(fact|rule)[ \t]+[a-z0-9-]+/ {
	s = comment
	while (match(s, /@(fact|rule)[ \t]+[a-z0-9-]+/)) {
		t = substr(s, RSTART, RLENGTH)
		k = (t ~ /^@fact/) ? "fact" : "rule"
		sub(/^@(fact|rule)[ \t]+/, "", t)
		akind[++nanchors] = k
		aid[nanchors] = t
		s = substr(s, RSTART + RLENGTH)
	}
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
		# A tag before the entity (in the file's header) is the file's,
		# whose name is its entity's.
		e = entity
		if (e == "") {
			e = FILENAME
			sub(/.*\//, "", e)
			sub(/\.vhd$/, "", e)
		}
		print id "\t" file "\t" FNR "\t" e "\t" branch[depth]
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
