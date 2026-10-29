//! The facts list, `doc/facts.toml`, holds together with the repository
//! (see `ceres_test_runner::facts`).

use ceres_test_runner::facts::{load, problems, repo_root};

#[test]
fn facts_hold_together() {
    let root = repo_root();
    let facts = load(&root).expect("doc/facts.toml");
    let problems = problems(&root, &facts).expect("the repository");
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn races_hold_together() {
    let root = repo_root();
    let facts = load(&root).expect("doc/facts.toml");
    let races = ceres_test_runner::races::load(&root).expect("hdl/races.toml");
    let problems = ceres_test_runner::races::problems(&races, &facts);
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn rules_hold_together() {
    let root = repo_root();
    let facts = load(&root).expect("doc/facts.toml");
    let races = ceres_test_runner::races::load(&root).expect("hdl/races.toml");
    let rules = ceres_test_runner::rules::load(&root).expect("hdl/rules.toml");
    let problems = ceres_test_runner::rules::problems(&rules, &facts, &races);
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
