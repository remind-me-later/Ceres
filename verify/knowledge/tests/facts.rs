//! The facts list, `verify/knowledge/facts.toml`, holds together with the repository
//! (see `ceres_knowledge::facts`).

use ceres_knowledge::facts::{load, problems, repo_root};

#[test]
fn facts_hold_together() {
    let root = repo_root();
    let facts = load(&root).expect("verify/knowledge/facts.toml");
    let problems = problems(&root, &facts).expect("the repository");
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn assumptions_parse() {
    let root = repo_root();
    let assumptions =
        ceres_knowledge::assumptions::load(&root).expect("verify/knowledge/assumptions.toml");
    assert!(!assumptions.is_empty(), "no assumptions");
    ceres_knowledge::assumptions::scan(&root).expect("ceres-core's sources");
}

#[test]
fn races_hold_together() {
    let root = repo_root();
    let facts = load(&root).expect("verify/knowledge/facts.toml");
    let races = ceres_knowledge::races::load(&root).expect("verify/knowledge/races.toml");
    let problems = ceres_knowledge::races::problems(&races, &facts);
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn rules_hold_together() {
    let root = repo_root();
    let facts = load(&root).expect("verify/knowledge/facts.toml");
    let races = ceres_knowledge::races::load(&root).expect("verify/knowledge/races.toml");
    let rules = ceres_knowledge::rules::load(&root).expect("verify/knowledge/rules.toml");
    let problems = ceres_knowledge::rules::problems(&rules, &facts, &races);
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
