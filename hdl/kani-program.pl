#!/usr/bin/env perl
# A harness's goto program, as `goto-instrument --show-goto-functions`
# dumps it, in a canonical form (hdl/kani.sh hashes it): two compilations of
# the same program give the same text, whatever else changed in the crate.
#
#   goto-instrument --show-goto-functions HARNESS.out | kani-program.pl
#
# rustc numbers the crate's static allocations and Kani its globals across
# the whole crate (`AllocId(N, ...)`, `ceres_core::global::N`), so an edit
# anywhere renumbers them. They are renamed in the order they first appear
# in the functions' bodies, the functions taken in their names' order (which
# no number enters), `__CPROVER_initialize` left out of that count: it
# assigns every global its initial value, in the order of their numbered
# names, so its lines are sorted once renamed. The source locations (file,
# line, column) are dropped: they label the results, and do not change what
# CBMC checks. Two programs that come out the same here differ at most in
# those names, consistently renamed, and in their locations.
use strict;
use warnings;

my (@blocks, $block);
while (my $line = <STDIN>) {
    next if $line =~ /^Reading GOTO program/;
    if ($line =~ /^\^+$/) {
        $block = [];
        push @blocks, $block;
        next;
    }
    next unless $block;
    # An instruction's location: "// 12 file F line L column C function G".
    $line =~ s{^(\s*// \d+) .*$}{$1};
    push @$block, $line;
}

my $name = qr/AllocId\(\d+, \w+\)|ceres_core::global::\d+/;
my (%canonical, $next);
$next = 0;
my $rename = sub {
    my ($text, $count) = @_;
    $text =~ s{($name)}{
        my $found = $1;
        my $kind = $found =~ /^AllocId/ ? 'alloc' : 'global';
        $canonical{$found} //= ($count ? "$kind#" . $next++ : "$kind?$found");
    }ge;
    return $text;
};

# First appearances: the functions in their names' order, but the initializer.
my @named = sort { $a->[0] cmp $b->[0] } grep { $_->[0] !~ /^__CPROVER_initialize / } @blocks;
for my $block (@named) {
    $rename->($_, 1) for @$block;
}
# What only the initializer names, in its sorted order.
for my $block (grep { $_->[0] =~ /^__CPROVER_initialize / } @blocks) {
    $rename->($_, 1) for sort @$block;
}

my @out;
for my $block (@blocks) {
    my @lines = map { $rename->($_, 0) } @$block;
    if ($block->[0] =~ /^__CPROVER_initialize /) {
        my $head = shift @lines;
        # The instruction numbers follow the order: drop them before sorting.
        @lines = ($head, sort map { (my $l = $_) =~ s{^\s*// \d+$}{}; $l } @lines);
    }
    push @out, join('', @lines);
}
print for sort @out;
