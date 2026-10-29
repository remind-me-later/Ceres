//! The races of the readable model, `hdl/races.toml`.
//!
//! Each entry is a race of `hdl/rtl`: a delay (an `after` clause) or a short
//! pulse that a flip-flop or a latch sees (`hdl/find-runts.sh`), and whether
//! the outcome hangs on the order of two signals or on the widths of their
//! paths. The knowledge database (`hdl/knowledge.sh`) checks that every
//! delay and every short pulse at a control has its entry; [`problems`]
//! checks the entries against the facts.

use {
    crate::facts::{Facts, ModelName, Race},
    anyhow::{Context as _, Result},
    serde::Deserialize,
    std::{collections::BTreeMap, fs, path::Path},
};

/// One entry of `hdl/races.toml` (its header says what each field means).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mechanism {
    pub title: String,
    pub models: Vec<ModelName>,
    pub race: Race,
    #[serde(default)]
    pub delays: Vec<String>,
    #[serde(default)]
    pub signals: Vec<String>,
    #[serde(default)]
    pub facts: Vec<String>,
    pub evidence: String,
}

/// The races, by id.
pub type Races = BTreeMap<String, Mechanism>;

/// Reads `hdl/races.toml`.
pub fn load(root: &Path) -> Result<Races> {
    let path = root.join("hdl").join("races.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// What does not hold together with the facts: one line per problem.
#[must_use]
pub fn problems(races: &Races, facts: &Facts) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, race) in races {
        let mut problem = |message: String| problems.push(format!("{id}: {message}"));
        if race.models.is_empty() {
            problem("no models".to_owned());
        }
        if race.delays.is_empty() && race.signals.is_empty() {
            problem("names no delay and no signal".to_owned());
        }
        for name in race.delays.iter().chain(&race.signals) {
            if name != "tco"
                && name
                    .split_once('.')
                    .is_none_or(|(e, s)| e.is_empty() || s.is_empty())
            {
                problem(format!("{name} is not entity.signal"));
            }
        }
        if race.race == Race::Width && race.facts.is_empty() {
            problem("a width race with no fact".to_owned());
        }
        for fact_id in &race.facts {
            match facts.get(fact_id) {
                None => problem(format!("no fact {fact_id}")),
                Some(fact) if fact.race != Some(race.race) => problem(format!(
                    "the fact {fact_id} has race {}, not {}",
                    fact.race.map_or("none", Race::name),
                    race.race.name()
                )),
                Some(fact)
                    if !fact
                        .models
                        .iter()
                        .any(|f| race.models.iter().any(|r| r.overlaps(*f))) =>
                {
                    problem(format!("the fact {fact_id} is about none of its models"));
                }
                Some(_) => {}
            }
        }
    }
    problems
}
