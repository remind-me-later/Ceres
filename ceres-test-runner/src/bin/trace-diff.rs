//! Trace diff tool for comparing execution traces
//!
//! This tool compares two trace files to find differences in execution,
//! useful for comparing passing vs failing test runs.

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "trace-diff")]
#[command(about = "Compare two execution traces to find differences", long_about = None)]
struct Cli {
    /// First trace file (JSONL format)
    #[arg(short = 'a', long = "trace-a")]
    trace_a: PathBuf,

    /// Second trace file (JSONL format)
    #[arg(short = 'b', long = "trace-b")]
    trace_b: PathBuf,

    /// Maximum differences to show
    #[arg(short = 'n', long, default_value_t = 20)]
    max_diffs: usize,

    /// Show context lines around differences
    #[arg(short, long, default_value_t = 2)]
    context: usize,

    /// Compare only specific fields (comma-separated: pc,instruction,registers,all)
    #[arg(short, long, default_value = "all")]
    fields: String,

    /// Stop at first difference (useful for debugging divergence point)
    #[arg(short, long)]
    stop_at_first: bool,
}

#[derive(Debug, Clone)]
struct TraceEntry {
    pc: u16,
    instruction: String,
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
}

impl TraceEntry {
    #[expect(clippy::cast_possible_truncation)]
    fn from_json(json: &serde_json::Value) -> Option<Self> {
        Some(Self {
            pc: json.get("pc")?.as_u64()? as u16,
            instruction: json.get("instruction")?.as_str()?.to_string(),
            a: json.get("a")?.as_u64()? as u8,
            f: json.get("f")?.as_u64()? as u8,
            b: json.get("b")?.as_u64()? as u8,
            c: json.get("c")?.as_u64()? as u8,
            d: json.get("d")?.as_u64()? as u8,
            e: json.get("e")?.as_u64()? as u8,
            h: json.get("h")?.as_u64()? as u8,
            l: json.get("l")?.as_u64()? as u8,
            sp: json.get("sp")?.as_u64()? as u16,
        })
    }

    fn format_registers(&self) -> String {
        format!(
            "A={:#04X} F={:#04X} B={:#04X} C={:#04X} D={:#04X} E={:#04X} H={:#04X} L={:#04X} SP={:#06X}",
            self.a, self.f, self.b, self.c, self.d, self.e, self.h, self.l, self.sp
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DiffField {
    Pc,
    Instruction,
    Registers,
}

#[derive(Debug)]
struct Difference {
    line_num: usize,
    field: DiffField,
    entry_a: TraceEntry,
    entry_b: TraceEntry,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("Comparing traces:");
    println!("  A: {}", cli.trace_a.display());
    println!("  B: {}", cli.trace_b.display());
    println!();

    // Parse field filter
    let fields = parse_fields(&cli.fields)?;

    // Load both traces
    let traces_a = load_trace(&cli.trace_a).context("Failed to load trace A")?;
    let traces_b = load_trace(&cli.trace_b).context("Failed to load trace B")?;

    println!("Trace A: {} entries", traces_a.len());
    println!("Trace B: {} entries\n", traces_b.len());

    // Find differences
    let differences = find_differences(&traces_a, &traces_b, &fields);

    if differences.is_empty() {
        println!("✓ Traces are identical!");
        return Ok(());
    }

    println!("Found {} difference(s)\n", differences.len());
    println!("{}", "=".repeat(80));

    // Display differences
    let to_show = if cli.stop_at_first {
        1
    } else {
        cli.max_diffs.min(differences.len())
    };

    for (i, diff) in differences.iter().take(to_show).enumerate() {
        if i > 0 {
            println!("\n{}", "-".repeat(80));
        }

        println!("Difference #{} at line {}:", i + 1, diff.line_num);
        println!("  Field: {:?}", diff.field);
        println!();

        // Show context
        let start = diff.line_num.saturating_sub(cli.context);
        let end = (diff.line_num + cli.context + 1)
            .min(traces_a.len())
            .min(traces_b.len());

        for line_num in start..end {
            let marker = if line_num == diff.line_num {
                ">>>"
            } else {
                "   "
            };

            if line_num < traces_a.len() {
                let entry = &traces_a[line_num];
                println!(
                    "{} A [{}] PC={:#06X} {}",
                    marker, line_num, entry.pc, entry.instruction
                );
                if line_num == diff.line_num {
                    println!("         {}", entry.format_registers());
                }
            }

            if line_num < traces_b.len() {
                let entry = &traces_b[line_num];
                println!(
                    "{} B [{}] PC={:#06X} {}",
                    marker, line_num, entry.pc, entry.instruction
                );
                if line_num == diff.line_num {
                    println!("         {}", entry.format_registers());
                }
            }

            if line_num == diff.line_num {
                println!();
                print_field_diff(&diff.entry_a, &diff.entry_b, diff.field);
            }
        }
    }

    if differences.len() > to_show {
        println!("\n{}", "=".repeat(80));
        println!("... and {} more difference(s)", differences.len() - to_show);
        println!("Use -n {} to see all differences", differences.len());
    }

    // Statistics
    println!("\n{}", "=".repeat(80));
    print_diff_statistics(&differences);

    Ok(())
}

fn load_trace(path: &PathBuf) -> Result<Vec<TraceEntry>> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut entries = Vec::new();
    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let json: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse line {line_num}"))?;

        if let Some(entry) = TraceEntry::from_json(&json) {
            entries.push(entry);
        } else {
            eprintln!("Warning: Skipping line {line_num} (incomplete data)");
        }
    }

    Ok(entries)
}

fn parse_fields(spec: &str) -> Result<Vec<DiffField>> {
    if spec == "all" {
        return Ok(vec![
            DiffField::Pc,
            DiffField::Instruction,
            DiffField::Registers,
        ]);
    }

    let mut fields = Vec::new();
    for part in spec.split(',') {
        match part.trim().to_lowercase().as_str() {
            "pc" => fields.push(DiffField::Pc),
            "instruction" | "inst" => fields.push(DiffField::Instruction),
            "registers" | "regs" => fields.push(DiffField::Registers),
            other => {
                anyhow::bail!("Unknown field: {other}. Valid: pc, instruction, registers, all")
            }
        }
    }

    if fields.is_empty() {
        anyhow::bail!("No fields specified");
    }

    Ok(fields)
}

fn find_differences(
    traces_a: &[TraceEntry],
    traces_b: &[TraceEntry],
    fields: &[DiffField],
) -> Vec<Difference> {
    let mut differences = Vec::new();
    let min_len = traces_a.len().min(traces_b.len());

    for i in 0..min_len {
        let entry_a = &traces_a[i];
        let entry_b = &traces_b[i];

        for &field in fields {
            let differs = match field {
                DiffField::Pc => entry_a.pc != entry_b.pc,
                DiffField::Instruction => entry_a.instruction != entry_b.instruction,
                DiffField::Registers => {
                    entry_a.a != entry_b.a
                        || entry_a.f != entry_b.f
                        || entry_a.b != entry_b.b
                        || entry_a.c != entry_b.c
                        || entry_a.d != entry_b.d
                        || entry_a.e != entry_b.e
                        || entry_a.h != entry_b.h
                        || entry_a.l != entry_b.l
                        || entry_a.sp != entry_b.sp
                }
            };

            if differs {
                differences.push(Difference {
                    line_num: i,
                    field,
                    entry_a: entry_a.clone(),
                    entry_b: entry_b.clone(),
                });
            }
        }
    }

    differences
}

fn print_field_diff(entry_a: &TraceEntry, entry_b: &TraceEntry, field: DiffField) {
    match field {
        DiffField::Pc => {
            println!("  PC differs:");
            println!("    A: {:#06X}", entry_a.pc);
            println!("    B: {:#06X}", entry_b.pc);
        }
        DiffField::Instruction => {
            println!("  Instruction differs:");
            println!("    A: {}", entry_a.instruction);
            println!("    B: {}", entry_b.instruction);
        }
        DiffField::Registers => {
            println!("  Register differences:");
            if entry_a.a != entry_b.a {
                println!("    A: {:#04X} vs {:#04X}", entry_a.a, entry_b.a);
            }
            if entry_a.f != entry_b.f {
                println!("    F: {:#04X} vs {:#04X}", entry_a.f, entry_b.f);
            }
            if entry_a.b != entry_b.b {
                println!("    B: {:#04X} vs {:#04X}", entry_a.b, entry_b.b);
            }
            if entry_a.c != entry_b.c {
                println!("    C: {:#04X} vs {:#04X}", entry_a.c, entry_b.c);
            }
            if entry_a.d != entry_b.d {
                println!("    D: {:#04X} vs {:#04X}", entry_a.d, entry_b.d);
            }
            if entry_a.e != entry_b.e {
                println!("    E: {:#04X} vs {:#04X}", entry_a.e, entry_b.e);
            }
            if entry_a.h != entry_b.h {
                println!("    H: {:#04X} vs {:#04X}", entry_a.h, entry_b.h);
            }
            if entry_a.l != entry_b.l {
                println!("    L: {:#04X} vs {:#04X}", entry_a.l, entry_b.l);
            }
            if entry_a.sp != entry_b.sp {
                println!("    SP: {:#06X} vs {:#06X}", entry_a.sp, entry_b.sp);
            }
        }
    }
}

fn print_diff_statistics(differences: &[Difference]) {
    println!("Difference Statistics:");

    let mut field_counts = HashMap::new();
    for diff in differences {
        *field_counts.entry(diff.field).or_insert(0) += 1;
    }

    for (field, count) in &field_counts {
        println!("  {field:?}: {count} difference(s)");
    }
}
