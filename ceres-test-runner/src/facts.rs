//! The facts list, `doc/facts.toml`.
//!
//! It lists the behaviours of the hardware whose evidence is missing or
//! contradictory, where ceres departs from the DMG-CPU B's netlist, or whose
//! outcome hangs on a race, and what each rests on.
//!
//! [`problems`] checks that the list holds together with the rest of the
//! repository: every departure from the netlist rests on a hardware test or
//! is an open question, the test ROMs and the open questions it names
//! exist, every open question of the documents it points to is one of its
//! facts, and every `@fact` tag in the code names a fact. The test `facts`
//! runs it, and the example `facts` prints the lists it gives.

use {
    anyhow::{Context as _, Result},
    serde::Deserialize,
    std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
    },
};

/// The models a fact is about.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ModelName {
    Dmg,
    Mgb,
    Sgb,
    Cgb,
    CgbC,
    CgbE,
}

impl ModelName {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dmg => "dmg",
            Self::Mgb => "mgb",
            Self::Sgb => "sgb",
            Self::Cgb => "cgb",
            Self::CgbC => "cgb-c",
            Self::CgbE => "cgb-e",
        }
    }

    /// The two name the same model, or one is the CGB and the other one of
    /// its revisions.
    #[must_use]
    pub fn overlaps(self, other: Self) -> bool {
        self == other
            || matches!(
                (self, other),
                (Self::Cgb, Self::CgbC | Self::CgbE) | (Self::CgbC | Self::CgbE, Self::Cgb)
            )
    }

    /// The model has the DMG-CPU's logic (the MGB's and SGB's as a working
    /// assumption), so the netlist speaks for it.
    #[must_use]
    pub const fn has_netlist(self) -> bool {
        matches!(self, Self::Dmg | Self::Mgb | Self::Sgb)
    }
}

/// What the netlist (`hdl/rtl`, dmg-sim) does, against ceres.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Netlist {
    Same,
    Differs,
    /// It cannot tell: the outcome is electrical (two drivers, the RAM's
    /// cells), the power-on state, or outside the chip.
    Silent,
    /// No netlist: CGB only.
    None,
}

impl Netlist {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::Differs => "differs",
            Self::Silent => "silent",
            Self::None => "none",
        }
    }
}

/// What the hardware-verified test ROMs (`tests`) show, against ceres.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Hardware {
    Same,
    Differs,
    /// No test looks.
    None,
}

impl Hardware {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::Differs => "differs",
            Self::None => "none",
        }
    }
}

/// What the other emulators (SameBoy, Gambatte) do, where it matters.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Emulators {
    /// They do what ceres does.
    Agree,
    /// They differ, from ceres or from each other.
    Disagree,
}

impl Emulators {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Agree => "agree",
            Self::Disagree => "disagree",
        }
    }
}

/// How the outcome hangs on the order of two signals.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Race {
    /// The two are a phase or more apart: any order-keeping model gets it.
    Order,
    /// A pulse of almost no width, or two paths of almost the same delay:
    /// the die's physics decides, and it can vary by unit and revision.
    Width,
}

impl Race {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Order => "order",
            Self::Width => "width",
        }
    }
}

/// What settling an open question takes.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Need {
    FlashCart,
    LogicAnalyserLcd,
    LogicAnalyserCartridge,
    Scope,
    Camera,
    DmgB,
    DmgC,
    CgbC,
    CgbE,
    SeveralUnits,
}

impl Need {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::FlashCart => "flash-cart",
            Self::LogicAnalyserLcd => "logic-analyser-lcd",
            Self::LogicAnalyserCartridge => "logic-analyser-cartridge",
            Self::Scope => "scope",
            Self::Camera => "camera",
            Self::DmgB => "dmg-b",
            Self::DmgC => "dmg-c",
            Self::CgbC => "cgb-c",
            Self::CgbE => "cgb-e",
            Self::SeveralUnits => "several-units",
        }
    }
}

/// One entry of `doc/facts.toml` (its header says what each field means).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub title: String,
    pub models: Vec<ModelName>,
    pub netlist: Netlist,
    pub hardware: Hardware,
    pub emulators: Option<Emulators>,
    pub race: Option<Race>,
    #[serde(default)]
    pub tests: Vec<String>,
    #[serde(default)]
    pub open: Vec<String>,
    #[serde(default)]
    pub needs: Vec<Need>,
    pub note: Option<String>,
}

/// A `@fact <id>` tag in a source file.
#[derive(Debug)]
pub struct Tag {
    pub id: String,
    pub file: PathBuf,
    pub line: usize,
}

/// The facts, by id.
pub type Facts = BTreeMap<String, Fact>;

/// The repository's root.
#[must_use]
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Reads `doc/facts.toml`.
pub fn load(root: &Path) -> Result<Facts> {
    let path = root.join("doc").join("facts.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// The directories searched for `@fact` tags, and the ones skipped in them.
const TAGGED: [&str; 5] = ["ceres-core", "ceres-std", "ceres-test-runner", "hdl", "doc"];
const UNTAGGED: [&str; 4] = ["target", "build", "external", ".git"];
const TAGGED_EXTENSIONS: [&str; 6] = ["rs", "vhd", "asm", "sh", "md", "txt"];

/// The `@fact` tags in the code and the documents.
pub fn tags(root: &Path) -> Result<Vec<Tag>> {
    let mut tags = Vec::new();
    for dir in TAGGED {
        collect_tags(root, &root.join(dir), &mut tags)?;
    }
    Ok(tags)
}

fn collect_tags(root: &Path, dir: &Path, tags: &mut Vec<Tag>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if !UNTAGGED.contains(&name) {
                collect_tags(root, &path, tags)?;
            }
            continue;
        }
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !TAGGED_EXTENSIONS.contains(&extension) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let file = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        for (index, line) in text.lines().enumerate() {
            for id in tag_ids(line) {
                tags.push(Tag {
                    id: id.to_owned(),
                    file: file.clone(),
                    line: index + 1,
                });
            }
        }
    }
    Ok(())
}

/// The ids that the `@fact` tags of a line name. A tag without an id (as
/// in `@fact <id>`) is not one.
fn tag_ids(line: &str) -> impl Iterator<Item = &str> {
    line.split("@fact ").skip(1).filter_map(|rest| {
        let end = rest
            .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
            .unwrap_or(rest.len());
        (end > 0).then(|| &rest[..end])
    })
}

/// GitHub's anchor for a heading: lower case, spaces to hyphens, and only
/// letters, digits, hyphens and underscores kept.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

/// The headings of a Markdown file (outside code blocks), with their level.
fn headings(text: &str) -> Vec<(usize, &str)> {
    let mut fenced = false;
    let mut headings = Vec::new();
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with('#') {
            let level = line.chars().take_while(|&c| c == '#').count();
            headings.push((level, line[level..].trim()));
        } else {
            // Not a heading.
        }
    }
    headings
}

/// The anchors of the entries under the level 2 headings that begin "Open
/// questions" (level 3 under those).
fn open_questions(text: &str) -> Vec<String> {
    let mut inside = false;
    let mut questions = Vec::new();
    for (level, heading) in headings(text) {
        if level == 2 {
            inside = heading.starts_with("Open questions");
        } else if inside && level == 3 {
            questions.push(anchor(heading));
        } else {
            // Another level.
        }
    }
    questions
}

/// What does not hold together: one line per problem, empty if all is well.
pub fn problems(root: &Path, facts: &Facts) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    let roms = crate::test_roms_dir();
    let mut docs = BTreeMap::new();
    let mut referenced = BTreeSet::new();

    for (id, fact) in facts {
        let mut problem = |message: String| problems.push(format!("{id}: {message}"));

        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            problem("the id is not kebab-case".to_owned());
        }
        if fact.models.is_empty() {
            problem("no models".to_owned());
        }
        let netlist_applies = fact.models.iter().any(|m| m.has_netlist());
        if netlist_applies == (fact.netlist == Netlist::None) {
            problem(if netlist_applies {
                "netlist = \"none\" for a model the netlist speaks for".to_owned()
            } else {
                "a CGB-only fact has netlist = \"none\"".to_owned()
            });
        }
        let open = !fact.open.is_empty();
        let tested = fact.hardware == Hardware::Same && !fact.tests.is_empty();
        if fact.netlist == Netlist::Differs && !tested && !open {
            problem(
                "departs from the netlist with no hardware test and no open question".to_owned(),
            );
        }
        if fact.hardware == Hardware::Differs && !open {
            problem("ceres fails a hardware test and there is no open question".to_owned());
        }
        if fact.hardware != Hardware::None && fact.tests.is_empty() {
            problem("hardware is same or differs, but no test is listed".to_owned());
        }
        if open && fact.needs.is_empty() {
            problem("an open question with no needs".to_owned());
        }
        for test in &fact.tests {
            if !roms.join(test).exists() {
                problem(format!("no test ROM {test} under external/test-roms"));
            }
        }
        for link in &fact.open {
            let Some((file, fragment)) = link.split_once('#') else {
                problem(format!("{link} has no #anchor"));
                continue;
            };
            if !docs.contains_key(file) {
                let Ok(text) = fs::read_to_string(root.join(file)) else {
                    problem(format!("no document {file}"));
                    continue;
                };
                docs.insert(file.to_owned(), text);
            }
            if headings(&docs[file])
                .iter()
                .all(|(_, heading)| anchor(heading) != fragment)
            {
                problem(format!("no heading #{fragment} in {file}"));
            }
            referenced.insert(link.clone());
        }
    }

    for (file, text) in &docs {
        for question in open_questions(text) {
            if !referenced.contains(&format!("{file}#{question}")) {
                problems.push(format!(
                    "{file}#{question}: an open question that no fact names"
                ));
            }
        }
    }

    for tag in tags(root)? {
        if !facts.contains_key(&tag.id) {
            problems.push(format!(
                "{}:{}: @fact {} names no fact",
                tag.file.display(),
                tag.line,
                tag.id
            ));
        }
    }

    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::{anchor, tag_ids};

    #[test]
    fn anchors_are_github_s() {
        assert_eq!(
            anchor("4. Enabling the timer with TAC"),
            "4-enabling-the-timer-with-tac"
        );
        assert_eq!(
            anchor("6. `halt_op_dupe_delay` and `stat_write_glitch_l154_d`"),
            "6-halt_op_dupe_delay-and-stat_write_glitch_l154_d"
        );
        assert_eq!(
            anchor("2. Where a read sees LY, the LY=LYC flag and the mode bits"),
            "2-where-a-read-sees-ly-the-lylyc-flag-and-the-mode-bits"
        );
    }

    #[test]
    fn tags_need_an_id() {
        // Split, so that the repository's check does not take it for a tag.
        let line = concat!("// @", "fact some-fact, @", "fact <id> and @", "fact x2");
        let ids: Vec<_> = tag_ids(line).collect();
        assert_eq!(ids, ["some-fact", "x2"]);
    }
}
