//! What the proofs against the netlist assume, `doc/assumptions.toml`, and
//! the `@assumes` and `@discharges` tags of ceres-core's Kani harnesses.
//!
//! Each entry is something a harness takes as given: a transcription of the
//! readable model, a mapping of ceres's time onto the netlist's, a
//! difference the facts record, or a case it leaves out. A harness names
//! what it assumes, and what it proves for the others, with a tag in the
//! comment before its `fn`. The knowledge base (`hdl/knowledge/knowledge.dl`)
//! follows them from harness to harness: an assumption that a harness
//! discharges brings in what that harness rests on.

use {
    anyhow::{Context as _, Result},
    serde::Deserialize,
    std::{
        collections::BTreeMap,
        fs, mem,
        path::{Path, PathBuf},
    },
};

/// What an assumption rests on.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Evidence {
    /// A harness proves it (`@discharges <id>`).
    Proved,
    /// A transcription of the readable model, which the replay of the
    /// model's traces checks (ceres-core's `netlist_replay`).
    Replayed,
    /// Measured: on the model's traces, on ceres's runs or on hardware.
    Measured,
    /// Ceres parts from the netlist there, as the facts record.
    Fact,
    /// Not known yet: a case the proofs leave out.
    Open,
}

impl Evidence {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Proved => "proved",
            Self::Replayed => "replayed",
            Self::Measured => "measured",
            Self::Fact => "fact",
            Self::Open => "open",
        }
    }
}

/// One entry of `doc/assumptions.toml` (its header says what each field
/// means).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assumption {
    pub title: String,
    pub evidence: Evidence,
    pub how: Option<String>,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub rests_on: Vec<String>,
}

/// The assumptions, by id.
pub type Assumptions = BTreeMap<String, Assumption>;

/// Reads `doc/assumptions.toml`.
pub fn load(root: &Path) -> Result<Assumptions> {
    let path = root.join("doc").join("assumptions.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// What a tag says a function does with an assumption.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
    /// `@assumes <id>`: the function takes it as given.
    Assumes,
    /// `@discharges <id>`: the function proves it.
    Discharges,
}

impl Claim {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Assumes => "assumes",
            Self::Discharges => "discharges",
        }
    }

    const ALL: [Self; 2] = [Self::Assumes, Self::Discharges];

    const fn tag(self) -> &'static str {
        match self {
            Self::Assumes => "@assumes ",
            Self::Discharges => "@discharges ",
        }
    }
}

/// A tag, and the function it is on: the first `fn` after it.
#[derive(Debug)]
pub struct Tag {
    pub claim: Claim,
    pub id: String,
    pub function: String,
    pub file: PathBuf,
    pub line: usize,
}

/// A Kani harness: a `fn` after `#[kani::proof]`.
#[derive(Debug)]
pub struct Harness {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
}

/// The tags and the harnesses in ceres-core's sources.
pub fn scan(root: &Path) -> Result<(Vec<Tag>, Vec<Harness>)> {
    let (mut tags, mut harnesses) = (Vec::new(), Vec::new());
    collect(
        root,
        &root.join("ceres-core").join("src"),
        &mut tags,
        &mut harnesses,
    )?;
    Ok((tags, harnesses))
}

fn collect(
    root: &Path,
    dir: &Path,
    tags: &mut Vec<Tag>,
    harnesses: &mut Vec<Harness>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, tags, harnesses)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let file = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        // The tags and the proof attribute wait for the next `fn`.
        let mut pending: Vec<(Claim, String, usize)> = Vec::new();
        let mut proof = false;
        for (index, line) in text.lines().enumerate() {
            for claim in Claim::ALL {
                for id in tag_ids(line, claim.tag()) {
                    pending.push((claim, id.to_owned(), index + 1));
                }
            }
            if line.trim() == "#[kani::proof]" {
                proof = true;
            }
            let Some(name) = function_name(line) else {
                continue;
            };
            for (claim, id, line) in mem::take(&mut pending) {
                tags.push(Tag {
                    claim,
                    id,
                    function: name.to_owned(),
                    file: file.clone(),
                    line,
                });
            }
            if proof {
                harnesses.push(Harness {
                    name: name.to_owned(),
                    file: file.clone(),
                    line: index + 1,
                });
                proof = false;
            }
        }
    }
    Ok(())
}

/// The ids that a line's tags name. A tag without an id (as in
/// `@assumes <id>`) is not one.
fn tag_ids<'a>(line: &'a str, tag: &'a str) -> impl Iterator<Item = &'a str> {
    line.split(tag).skip(1).filter_map(|rest| {
        let end = rest
            .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
            .unwrap_or(rest.len());
        (end > 0).then(|| &rest[..end])
    })
}

/// The name of the function a line declares, if it declares one.
fn function_name(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.starts_with("//") {
        return None;
    }
    let rest = &line[line.find("fn ")? + 3..];
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
    (end > 0 && rest[end..].starts_with(['(', '<'])).then(|| &rest[..end])
}

#[cfg(test)]
mod tests {
    use super::{function_name, tag_ids};

    #[test]
    fn functions_are_found() {
        assert_eq!(
            function_name("fn netlist_scan_dma() {"),
            Some("netlist_scan_dma")
        );
        assert_eq!(
            function_name("    pub(crate) const fn runs(&self) -> bool {"),
            Some("runs")
        );
        assert_eq!(function_name("// fn not_this() {"), None);
        assert_eq!(function_name("let f = |x| x; // no fn here"), None);
    }

    #[test]
    fn tags_need_an_id() {
        let ids: Vec<_> = tag_ids("// @assumes a-b @assumes <id>", "@assumes ").collect();
        assert_eq!(ids, ["a-b"]);
    }
}
