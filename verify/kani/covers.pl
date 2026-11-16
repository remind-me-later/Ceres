#!/usr/bin/env perl
# The covers of a harness (kani.sh): "function<TAB>description" lines, a
# cover's function and the description its `kani::cover!` gives.
#
#   covers.pl program    from goto-instrument's --show-properties of the
#                        harness's goto program: the covers it contains
#   covers.pl results    from the harness's results (cargo kani's
#                        --output-into-files): with each cover's status
#                        (SATISFIED, UNSATISFIABLE or UNREACHABLE) last
#
# A cover is named by its function and description rather than by Kani's
# number in the function, which changes as covers are added before it.
use strict;
use warnings;

my $mode = shift // '';
my %seen;
my ($function, $status);
while (<>) {
    chomp;
    if ($mode eq 'program') {
        # Property timing::relation::step_agrees.cover.2:
        #   file ... function timing::relation::step_agrees
        #   [KANI_CHECK_ID_...] ends reloading
        if (/^Property (.+)\.cover\.\d+:$/) {
            $function = $1;
        } elsif (defined $function && /^\s*\[KANI_CHECK_ID_[^\]]*\] (.*)$/) {
            my $line = "$function\t$1";
            print "$line\n" unless $seen{$line}++;
            undef $function;
        }
    } elsif ($mode eq 'results') {
        # Check 12: timing::relation::step_agrees.cover.1
        #          - Status: SATISFIED
        #          - Description: "interrupt"
        if (/^Check \d+: (.+)\.cover\.\d+$/) {
            ($function, $status) = ($1, undef);
        } elsif (defined $function && /^\s*- Status: (\S+)/) {
            $status = $1;
        } elsif (defined $function && /^\s*- Description: "(.*)"$/) {
            # A description twice in a function: satisfied if either is.
            my $key = "$function\t$1";
            $seen{$key} = $status unless ($seen{$key} // '') eq 'SATISFIED';
            undef $function;
        }
    } else {
        die "usage: covers.pl program|results [FILE]\n";
    }
}
if ($mode eq 'results') {
    print "$_\t$seen{$_}\n" for sort keys %seen;
}
