/*
 * A VHPI plugin for nvc that lists the short pulses of the readable model:
 * a one-bit signal (or a bit of a vector) that changes and changes back
 * within RUNT_WIDTH nanoseconds (60 by default, half a phase), including
 * the pulses of no width, a delta cycle long, that a waveform dump leaves
 * out. A flip-flop clocked by such a pulse, or a latch it opens, takes it
 * or not by the die's physics: the model's outcome there hangs on its
 * delays (see find-runts.sh).
 *
 *   nvc -e ... -r --load=build/runts.so
 *
 * writes, as the simulation ends, a line for each bit that had such
 * pulses to RUNT_FILE (standard output by default): the number of pulses,
 * the shortest one's width in femtoseconds, the time the first began, the
 * level of the pulses (1 high, 0 low, b both) and the bit's path.
 * RUNT_TRACE=text also prints each pulse of the bits whose path contains
 * the text (in capitals, as VHPI names them) to the standard error: the
 * time it began and its width, in femtoseconds, its level and the path.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <vhpi_user.h>

/* The positions of std_ulogic's values. */
enum { U, X, ZERO, ONE, Z, W, L, H, DC };

struct bit {
	int level;          /* 0, 1 or -1 (neither) */
	int before;         /* the level before the last change */
	uint64_t changed;   /* when it last changed, in femtoseconds */
	uint64_t pulses, shortest, first;
	int levels;         /* bit 0: low pulses, bit 1: high pulses */
};

struct sig {
	char *name;
	int width;          /* 0 for a scalar */
	int left, step;     /* the index of element 0, and the direction */
	vhpiHandleT handle;
	vhpiValueT value;
	struct bit *bits;
	int traced;
	struct sig *next;
};

static struct sig *sigs;
static uint64_t max_width = 60000000;  /* 60 ns */

static int level(int v)
{
	switch (v) {
	case ZERO: case L: return 0;
	case ONE: case H: return 1;
	default: return -1;
	}
}

static uint64_t now(void)
{
	vhpiTimeT t;
	long cycles;
	vhpi_get_time(&t, &cycles);
	return ((uint64_t)(uint32_t)t.high << 32) | t.low;
}

static void change(struct sig *s, int i, int v, uint64_t t)
{
	struct bit *b = &s->bits[i];
	int l = level(v);
	if (l == b->level)
		return;
	if (l >= 0 && b->level >= 0 && b->before == l && b->changed > 0 &&
	    t - b->changed < max_width) {
		uint64_t w = t - b->changed;
		if (b->pulses == 0)
			b->first = b->changed;
		if (b->pulses == 0 || w < b->shortest)
			b->shortest = w;
		b->pulses++;
		b->levels |= 1 << b->level;
		if (s->traced) {
			fprintf(stderr, "runt %llu %llu %d %s", (unsigned long long)b->changed,
				(unsigned long long)w, b->level, s->name);
			if (s->width)
				fprintf(stderr, "(%d)", s->left + s->step * i);
			fputc('\n', stderr);
		}
	}
	b->before = b->level;
	b->level = l;
	b->changed = t;
}

static void value_change(const vhpiCbDataT *cb)
{
	struct sig *s = cb->user_data;
	uint64_t t = now();
	if (vhpi_get_value(s->handle, &s->value) != 0)
		return;
	if (s->width == 0)
		change(s, 0, s->value.value.enumv, t);
	else
		for (int i = 0; i < s->width; i++)
			change(s, i, s->value.value.enumvs[i], t);
}

/* Whether a type is std_ulogic or one of its subtypes. */
static int is_logic(vhpiHandleT type)
{
	vhpiHandleT base = vhpi_handle(vhpiBaseType, type);
	if (base == NULL)
		base = type;
	const char *name = (const char *)vhpi_get_str(vhpiNameP, base);
	return name != NULL && (strcasecmp(name, "STD_ULOGIC") == 0);
}

static void watch(vhpiHandleT h)
{
	vhpiHandleT type = vhpi_handle(vhpiType, h);
	if (type == NULL)
		return;
	vhpiHandleT base = vhpi_handle(vhpiBaseType, type);
	if (base == NULL)
		base = type;
	int width = 0;
	vhpiFormatT format;
	if (vhpi_get(vhpiKindP, base) == vhpiArrayTypeDeclK) {
		vhpiHandleT elem = vhpi_handle(vhpiElemType, base);
		if (elem == NULL || !is_logic(elem) || vhpi_get(vhpiNumDimensionsP, base) != 1)
			return;
		width = vhpi_get(vhpiSizeP, h);
		if (width <= 0)
			return;
		format = vhpiLogicVecVal;
	} else if (is_logic(type)) {
		format = vhpiLogicVal;
	} else {
		return;
	}

	struct sig *s = calloc(1, sizeof *s);
	s->name = strdup((const char *)vhpi_get_str(vhpiFullNameP, h));
	s->width = width;
	s->handle = h;
	s->value.format = format;
	s->left = 0;
	s->step = 1;
	if (width > 0) {
		s->value.bufSize = width * sizeof(vhpiEnumT);
		s->value.value.enumvs = calloc(width, sizeof(vhpiEnumT));
		/* The bits' indices, for the names. */
		vhpiHandleT ranges = vhpi_iterator(vhpiConstraints, vhpi_handle(vhpiType, h));
		vhpiHandleT range = ranges ? vhpi_scan(ranges) : NULL;
		if (range != NULL) {
			s->left = vhpi_get(vhpiLeftBoundP, range);
			s->step = vhpi_get(vhpiIsUpP, range) ? 1 : -1;
		}
	}
	s->bits = calloc(width ? width : 1, sizeof *s->bits);
	int ok = vhpi_get_value(h, &s->value) == 0;
	for (int i = 0; i < (width ? width : 1); i++) {
		int v = width ? s->value.value.enumvs[i] : s->value.value.enumv;
		s->bits[i].level = ok ? level(v) : -1;
		s->bits[i].before = -1;
	}
	const char *trace = getenv("RUNT_TRACE");
	s->traced = trace != NULL && strstr(s->name, trace) != NULL;
	s->next = sigs;
	sigs = s;

	vhpiCbDataT cb = {
		.reason = vhpiCbValueChange,
		.cb_rtn = value_change,
		.obj = h,
		.user_data = s,
	};
	vhpi_register_cb(&cb, vhpiReturnCb);
}

static void walk(vhpiHandleT region)
{
	vhpiHandleT it = vhpi_iterator(vhpiSigDecls, region);
	for (vhpiHandleT h; it && (h = vhpi_scan(it)); )
		watch(h);
	/* The ports too: a pulse shows under the name its reader knows. */
	it = vhpi_iterator(vhpiPortDecls, region);
	for (vhpiHandleT h; it && (h = vhpi_scan(it)); )
		watch(h);
	it = vhpi_iterator(vhpiInternalRegions, region);
	for (vhpiHandleT h; it && (h = vhpi_scan(it)); )
		walk(h);
}

static void report(const vhpiCbDataT *cb)
{
	(void)cb;
	const char *path = getenv("RUNT_FILE");
	FILE *f = path ? fopen(path, "w") : stdout;
	if (f == NULL)
		return;
	for (struct sig *s = sigs; s; s = s->next)
		for (int i = 0; i < (s->width ? s->width : 1); i++) {
			struct bit *b = &s->bits[i];
			if (b->pulses == 0)
				continue;
			fprintf(f, "%llu %llu %llu %c %s", (unsigned long long)b->pulses,
				(unsigned long long)b->shortest, (unsigned long long)b->first,
				"01b"[b->levels - 1], s->name);
			if (s->width)
				fprintf(f, "(%d)", s->left + s->step * i);
			fputc('\n', f);
		}
	if (f != stdout)
		fclose(f);
}

static void start(const vhpiCbDataT *cb)
{
	(void)cb;
	const char *w = getenv("RUNT_WIDTH");
	if (w != NULL)
		max_width = (uint64_t)(strtod(w, NULL) * 1e6);
	walk(vhpi_handle(vhpiRootInst, NULL));
	vhpiCbDataT end = { .reason = vhpiCbEndOfSimulation, .cb_rtn = report };
	vhpi_register_cb(&end, 0);
}

static void startup(void)
{
	vhpiCbDataT cb = { .reason = vhpiCbStartOfSimulation, .cb_rtn = start };
	vhpi_register_cb(&cb, 0);
}

void (*vhpi_startup_routines[])(void) = { startup, NULL };
