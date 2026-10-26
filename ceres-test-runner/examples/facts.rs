//! Prints the lists that the facts of `doc/facts.toml` give: the open
//! questions and what settling them takes, where ceres departs from the
//! netlist and what that rests on, the races, and where the code tags each
//! fact.
//!
//! Usage: `cargo run -p ceres-test-runner --example facts -- [open|netlist|races|tags]`
//! (all four without an argument).

use ceres_test_runner::facts::{Facts, Hardware, Need, Netlist, Race, load, repo_root, tags};
use std::collections::BTreeMap;

const USAGE: &str = "usage: facts [open|netlist|races|tags]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = repo_root();
    let facts = load(&root).expect("doc/facts.toml");
    match args.first().map(String::as_str) {
        None => {
            open(&facts);
            netlist(&facts);
            races(&facts);
            tagged(&facts);
        }
        Some("open") => open(&facts),
        Some("netlist") => netlist(&facts),
        Some("races") => races(&facts),
        Some("tags") => tagged(&facts),
        Some(_) => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// The open questions, then how many of them each need serves.
fn open(facts: &Facts) {
    println!("Open questions:");
    let mut served: BTreeMap<Need, Vec<&str>> = BTreeMap::new();
    for (id, fact) in facts.iter().filter(|(_, f)| !f.open.is_empty()) {
        let needs: Vec<_> = fact.needs.iter().map(|n| n.name()).collect();
        println!("  {id}: {}", fact.title);
        println!("      needs {}", needs.join(", "));
        for need in &fact.needs {
            served.entry(*need).or_default().push(id);
        }
    }
    println!("\nWhat to get, by how many open facts it serves:");
    let mut served: Vec<_> = served.into_iter().collect();
    served.sort_by_key(|(_, ids)| std::cmp::Reverse(ids.len()));
    for (need, ids) in served {
        println!("  {:26} {:2}", need.name(), ids.len());
    }
    println!();
}

/// Where ceres departs from the netlist, and what each departure rests on.
fn netlist(facts: &Facts) {
    println!("Where ceres departs from the netlist:");
    for (id, fact) in facts.iter().filter(|(_, f)| f.netlist == Netlist::Differs) {
        let rests_on = match (fact.hardware, fact.open.is_empty()) {
            (Hardware::Same, true) => "hardware tests".to_owned(),
            (Hardware::Same, false) => "hardware tests, open".to_owned(),
            (_, false) => "nothing yet: open".to_owned(),
            (_, true) => "nothing".to_owned(),
        };
        println!("  {id}: {} ({rests_on})", fact.title);
    }
    println!();
}

/// The facts whose outcome hangs on a race, the width races first.
fn races(facts: &Facts) {
    println!("Races:");
    let mut raced: Vec<_> = facts
        .iter()
        .filter_map(|(id, f)| f.race.map(|race| (race, id, f)))
        .collect();
    raced.sort_by_key(|(race, id, _)| (*race != Race::Width, *id));
    for (race, id, fact) in raced {
        println!("  {:5} {id}: {}", race.name(), fact.title);
    }
    println!();
}

/// Where the code tags each fact, and the facts no code tags.
fn tagged(facts: &Facts) {
    let tags = tags(&repo_root()).expect("the repository");
    println!("Tags:");
    for id in facts.keys() {
        let places: Vec<_> = tags
            .iter()
            .filter(|t| &t.id == id)
            .map(|t| format!("{}:{}", t.file.display(), t.line))
            .collect();
        if places.is_empty() {
            println!("  {id}: (untagged)");
        } else {
            println!("  {id}: {}", places.join(", "));
        }
    }
}
