//! The rules of the readable model, `hdl/rules.toml`.
//!
//! Each entry is a rule that `hdl/rtl` needs beyond the die's connectivity
//! to do what the hardware does: the outcome of a width race, a gate or a
//! latch the die does not show, what a RAM's cells or the analogue side do,
//! the power-on state, for the models it names. The knowledge database (`hdl/knowledge.sh`) checks
//! that every fact that needs a rule has one; [`problems`] checks the
//! entries against the facts and the races.

use {
    crate::{
        facts::{Facts, ModelName, Race},
        races::Races,
    },
    anyhow::{Context as _, Result},
    serde::Deserialize,
    std::{collections::BTreeMap, fs, path::Path},
};

/// What a rule changes.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A delay's value or range.
    Delay,
    /// Which way a pulse of no width goes.
    Outcome,
    /// A gate or a latch beyond the die's.
    Logic,
    /// What a RAM's cells do that the netlist does not model.
    Ram,
    /// The power-on state.
    State,
    /// The electrical side: buses with two drivers or none, the sound.
    Analogue,
    /// The mechanism is not known yet.
    Unknown,
}

impl Kind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Delay => "delay",
            Self::Outcome => "outcome",
            Self::Logic => "logic",
            Self::Ram => "ram",
            Self::State => "state",
            Self::Analogue => "analogue",
            Self::Unknown => "unknown",
        }
    }
}

/// Where a rule stands.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// `hdl/rtl` has it, tagged `@rule <id>`.
    Applied,
    /// Ceres does it and passes the tests; `hdl/rtl` does not have it yet.
    Proposed,
    /// Nothing known passes the tests.
    Missing,
}

impl Status {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Proposed => "proposed",
            Self::Missing => "missing",
        }
    }
}

/// One entry of `hdl/rules.toml` (its header says what each field means).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub title: String,
    pub models: Vec<ModelName>,
    pub kind: Kind,
    pub status: Status,
    #[serde(default)]
    pub races: Vec<String>,
    pub facts: Vec<String>,
    pub evidence: String,
}

/// The rules, by id.
pub type Rules = BTreeMap<String, Rule>;

/// Reads `hdl/rules.toml`.
pub fn load(root: &Path) -> Result<Rules> {
    let path = root.join("hdl").join("rules.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// What does not hold together with the facts and the races: one line per
/// problem.
#[must_use]
pub fn problems(rules: &Rules, facts: &Facts, races: &Races) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, rule) in rules {
        let mut problem = |message: String| problems.push(format!("{id}: {message}"));
        if rule.models.is_empty() {
            problem("no models".to_owned());
        }
        if rule.facts.is_empty() {
            problem("explains no fact".to_owned());
        }
        for fact_id in &rule.facts {
            match facts.get(fact_id) {
                None => problem(format!("no fact {fact_id}")),
                Some(fact)
                    if !fact
                        .models
                        .iter()
                        .any(|f| rule.models.iter().any(|r| r.overlaps(*f))) =>
                {
                    problem(format!("the fact {fact_id} is about none of its models"));
                }
                Some(_) => {}
            }
        }
        for race_id in &rule.races {
            match races.get(race_id) {
                None => problem(format!("no race {race_id}")),
                Some(race) if race.race != Race::Width => {
                    problem(format!("the race {race_id} is not a width race"));
                }
                Some(_) => {}
            }
        }
    }
    problems
}
