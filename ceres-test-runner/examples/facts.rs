//! Prints the lists that the facts of `doc/facts.toml` give: the open
//! questions and what settling them takes, where ceres departs from the
//! netlist and what that rests on, the races, and where the code tags each
//! fact. `tsv DIR` writes the facts, their tags, the races of
//! `hdl/races.toml` and the rules of `hdl/rules.toml` as tables, for the
//! knowledge database (`hdl/knowledge.sh`).
//!
//! Usage: `cargo run -p ceres-test-runner --example facts -- [open|netlist|races|tags|tsv DIR]`
//! (the first four without an argument).

use ceres_test_runner::{
    facts::{Emulators, Facts, Hardware, Need, Netlist, Race, load, repo_root, tags},
    races, rules,
};
use std::{collections::BTreeMap, fs, io::Write as _, path::Path};

const USAGE: &str = "usage: facts [open|netlist|races|tags|tsv DIR]";

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
        Some("tsv") if args.len() == 2 => tsv(&facts, Path::new(&args[1])),
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

/// A table for the knowledge database: a row per line, the fields apart by
/// tabs (which, like line breaks, the fields cannot hold).
fn table(dir: &Path, name: &str, rows: impl IntoIterator<Item = Vec<String>>) {
    let path = dir.join(format!("{name}.tsv"));
    let mut file = fs::File::create(&path).expect("the table");
    for row in rows {
        let fields: Vec<_> = row.iter().map(|f| f.replace(['\t', '\n'], " ")).collect();
        writeln!(file, "{}", fields.join("\t")).expect("the table");
    }
}

/// `entity.signal` as its two fields (`tco` stands for every flip-flop).
fn split_signal(name: &str) -> Vec<String> {
    let (entity, signal) = name.split_once('.').unwrap_or(("", name));
    vec![entity.to_owned(), signal.to_owned()]
}

/// The facts, their lists and tags, the races and the rules, as tables.
fn tsv(facts: &Facts, dir: &Path) {
    fs::create_dir_all(dir).expect("the directory");
    let opt = |s: Option<&str>| s.unwrap_or("").to_owned();
    table(
        dir,
        "fact",
        facts.iter().map(|(id, f)| {
            vec![
                id.clone(),
                f.title.clone(),
                f.netlist.name().to_owned(),
                f.hardware.name().to_owned(),
                opt(f.emulators.map(Emulators::name)),
                opt(f.race.map(Race::name)),
                opt(f.note.as_deref()),
            ]
        }),
    );
    let each = |list: fn(&ceres_test_runner::facts::Fact) -> Vec<String>| {
        facts
            .iter()
            .flat_map(move |(id, f)| list(f).into_iter().map(move |v| vec![id.clone(), v]))
            .collect::<Vec<_>>()
    };
    table(
        dir,
        "fact_model",
        each(|f| f.models.iter().map(|m| m.name().to_owned()).collect()),
    );
    table(dir, "fact_test", each(|f| f.tests.clone()));
    table(dir, "fact_open", each(|f| f.open.clone()));
    table(
        dir,
        "fact_need",
        each(|f| f.needs.iter().map(|n| n.name().to_owned()).collect()),
    );
    let tags = tags(&repo_root()).expect("the repository");
    table(
        dir,
        "fact_tag",
        tags.iter().map(|t| {
            vec![
                t.id.clone(),
                t.file.display().to_string(),
                t.line.to_string(),
            ]
        }),
    );
    race_tables(dir);
    rule_tables(dir);
}

/// The races and the signals and delays each covers, as tables.
fn race_tables(dir: &Path) {
    let races = races::load(&repo_root()).expect("hdl/races.toml");
    table(
        dir,
        "race",
        races.iter().map(|(id, r)| {
            vec![
                id.clone(),
                r.title.clone(),
                r.race.name().to_owned(),
                r.evidence.clone(),
            ]
        }),
    );
    let named = |names: fn(&races::Mechanism) -> &Vec<String>| {
        races
            .iter()
            .flat_map(move |(id, r)| {
                names(r).iter().map(move |n| {
                    let mut row = vec![id.clone()];
                    row.extend(split_signal(n));
                    row
                })
            })
            .collect::<Vec<_>>()
    };
    table(
        dir,
        "race_model",
        races.iter().flat_map(|(id, r)| {
            r.models
                .iter()
                .map(move |m| vec![id.clone(), m.name().to_owned()])
        }),
    );
    table(
        dir,
        "race_fact",
        races
            .iter()
            .flat_map(|(id, r)| r.facts.iter().map(move |f| vec![id.clone(), f.clone()])),
    );
    table(dir, "race_delay", named(|r| &r.delays));
    table(dir, "race_signal", named(|r| &r.signals));
}

/// The rules and the facts and races each explains, as tables.
fn rule_tables(dir: &Path) {
    let rules = rules::load(&repo_root()).expect("hdl/rules.toml");
    table(
        dir,
        "rule",
        rules.iter().map(|(id, r)| {
            vec![
                id.clone(),
                r.title.clone(),
                r.kind.name().to_owned(),
                r.status.name().to_owned(),
                r.evidence.clone(),
            ]
        }),
    );
    let each_rule = |list: fn(&rules::Rule) -> &Vec<String>| {
        rules
            .iter()
            .flat_map(move |(id, r)| list(r).iter().map(move |v| vec![id.clone(), v.clone()]))
            .collect::<Vec<_>>()
    };
    table(
        dir,
        "rule_model",
        rules.iter().flat_map(|(id, r)| {
            r.models
                .iter()
                .map(move |m| vec![id.clone(), m.name().to_owned()])
        }),
    );
    table(dir, "rule_fact", each_rule(|r| &r.facts));
    table(dir, "rule_race", each_rule(|r| &r.races));
}
