//! Compares two traces of signal changes, such as the pin traces of the VHDL
//! model and of dmg-sim.
//!
//! ```text
//! hdl-trace-diff REFERENCE TRACE [--from PS]
//! ```
//!
//! A trace has one change per line: `<time in ps>[ ps] <signal> <value>`,
//! with the values `0`, `1`, `x` and `z`. For each signal, the changes from
//! `--from` up to the end of the shorter trace must be the same, at the same
//! picosecond. Changes to the same value, and several changes in one time
//! step, count as one. Exits with 1 if the traces differ.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead as _, BufReader};
use std::process::ExitCode;

use anyhow::{Context as _, Result, bail};

/// The changes of each signal: (time, value).
type Changes = BTreeMap<String, Vec<(u64, char)>>;

fn load(path: &str) -> Result<(Changes, u64)> {
    let f = File::open(path).with_context(|| format!("opening {path}"))?;
    let mut changes = Changes::new();
    let mut end = 0;
    for (n, line) in BufReader::new(f).lines().enumerate() {
        let line = line?;
        let fields: Vec<&str> = line.split_whitespace().filter(|f| *f != "ps").collect();
        let [t, name, v] = fields[..] else {
            bail!("{path}:{}: expected `<time> <signal> <value>`", n + 1);
        };
        let t: u64 = t
            .parse()
            .with_context(|| format!("{path}:{}: time", n + 1))?;
        let v = v.chars().next().unwrap_or('x');
        end = end.max(t);
        let l = changes.entry(name.to_owned()).or_default();
        match l.last_mut() {
            Some(last) if last.0 == t => last.1 = v,
            _ => l.push((t, v)),
        }
    }
    // Drop the changes to the same value.
    for l in changes.values_mut() {
        l.dedup_by(|b, a| a.1 == b.1);
    }
    Ok((changes, end))
}

/// The value at a time, if any change happened before.
fn value_at(l: &[(u64, char)], t: u64) -> Option<char> {
    let i = l.partition_point(|&(ct, _)| ct <= t);
    i.checked_sub(1).map(|i| l[i].1)
}

fn main() -> Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (paths, from) = match &args[..] {
        [a, b] => ((a, b), 0),
        [a, b, flag, t] if flag == "--from" => ((a, b), t.parse().context("--from")?),
        _ => bail!("usage: hdl-trace-diff REFERENCE TRACE [--from PS]"),
    };
    let (reference, ref_end) = load(paths.0)?;
    let (trace, trace_end) = load(paths.1)?;
    let end = ref_end.min(trace_end);

    let names: BTreeSet<&String> = reference.keys().chain(trace.keys()).collect();
    let mut differ = Vec::new();
    for name in &names {
        let a = reference.get(*name).map_or(&[][..], Vec::as_slice);
        let b = trace.get(*name).map_or(&[][..], Vec::as_slice);
        let times: BTreeSet<u64> = a
            .iter()
            .chain(b)
            .map(|&(t, _)| t)
            .filter(|&t| t >= from && t <= end)
            .chain([from])
            .collect();
        if let Some(&t) = times.iter().find(|&&t| value_at(a, t) != value_at(b, t)) {
            differ.push((t, *name, value_at(a, t), value_at(b, t)));
        }
    }
    differ.sort();
    println!(
        "{} signals, {} differ, from {from} ps to {end} ps",
        names.len(),
        differ.len()
    );
    for (t, name, a, b) in differ.iter().take(20) {
        let show = |v: &Option<char>| v.map_or_else(|| "-".to_owned(), String::from);
        println!("  {t} ps {name}: reference {}, trace {}", show(a), show(b));
    }
    Ok(if differ.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
