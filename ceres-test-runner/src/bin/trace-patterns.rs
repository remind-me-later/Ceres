//! Pattern detection utility for trace analysis
//!
//! This tool detects common patterns in execution traces:
//! - Infinite loops (same PC executing repeatedly)
//! - Timing anomalies (unusually long instruction sequences)
//! - Stuck states (no progress over many frames)

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "trace-patterns")]
#[command(about = "Detect execution patterns in traces", long_about = None)]
struct Cli {
    /// Path to the trace file (JSONL format)
    trace: PathBuf,

    /// Minimum loop iterations to report
    #[arg(short = 'l', long, default_value_t = 100)]
    min_loop_iterations: usize,

    /// Consecutive identical PCs to consider a tight loop
    #[arg(short = 't', long, default_value_t = 10)]
    tight_loop_threshold: usize,

    /// Show detailed loop information
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug)]
struct LoopPattern {
    start_line: usize,
    end_line: usize,
    instructions: Vec<String>,
    iteration_count: usize,
}

#[derive(Debug)]
struct TightLoop {
    line: usize,
    pc: u16,
    instruction: String,
    consecutive_count: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("Analyzing trace: {}\n", cli.trace.display());

    // Load trace
    let trace_data = load_trace_data(&cli.trace).context("Failed to load trace")?;

    println!("Total entries: {}\n", trace_data.len());
    println!("{}", "=".repeat(80));

    // Detect tight loops (same instruction repeating)
    let tight_loops = detect_tight_loops(&trace_data, cli.tight_loop_threshold);
    if tight_loops.is_empty() {
        println!("\n✓ No tight loops detected");
    } else {
        println!("\n🔄 TIGHT LOOPS DETECTED ({} found):", tight_loops.len());
        println!("{}", "-".repeat(80));
        for (i, loop_info) in tight_loops.iter().enumerate() {
            println!(
                "{}. Line {}: PC={:#06X} \"{}\" × {} times",
                i + 1,
                loop_info.line,
                loop_info.pc,
                loop_info.instruction,
                loop_info.consecutive_count
            );
        }
    }

    // Detect loop patterns (repeating sequences)
    println!("\n{}", "=".repeat(80));
    let loop_patterns = detect_loop_patterns(&trace_data, cli.min_loop_iterations);
    if loop_patterns.is_empty() {
        println!(
            "\n✓ No repeating loop patterns detected (threshold: {} iterations)",
            cli.min_loop_iterations
        );
    } else {
        println!(
            "\n🔁 LOOP PATTERNS DETECTED ({} found):",
            loop_patterns.len()
        );
        println!("{}", "-".repeat(80));
        for (i, pattern) in loop_patterns.iter().enumerate() {
            println!(
                "{}. Lines {}-{}: {} iterations of {} instruction(s)",
                i + 1,
                pattern.start_line,
                pattern.end_line,
                pattern.iteration_count,
                pattern.instructions.len()
            );
            if cli.verbose && !pattern.instructions.is_empty() {
                println!("   Instructions:");
                for inst in &pattern.instructions {
                    println!("     - {inst}");
                }
            }
        }
    }

    // PC distribution analysis
    println!("\n{}", "=".repeat(80));
    let pc_distribution = analyze_pc_distribution(&trace_data);
    println!("\n📊 PC DISTRIBUTION:");
    println!("{}", "-".repeat(80));
    let mut sorted_pcs: Vec<_> = pc_distribution.iter().collect();
    sorted_pcs.sort_by(|a, b| b.1.cmp(a.1));

    println!("Top 10 most executed PCs:");
    for (i, (pc, count)) in sorted_pcs.iter().take(10).enumerate() {
        #[expect(clippy::cast_precision_loss)]
        let percentage = (**count as f64 / trace_data.len() as f64) * 100.0;
        println!(
            "  {}. PC={:#06X}: {} times ({:.1}%)",
            i + 1,
            pc,
            count,
            percentage
        );
    }

    // Instruction frequency analysis
    println!("\n{}", "=".repeat(80));
    let inst_distribution = analyze_instruction_distribution(&trace_data);
    println!("\n📈 INSTRUCTION FREQUENCY:");
    println!("{}", "-".repeat(80));
    let mut sorted_insts: Vec<_> = inst_distribution.iter().collect();
    sorted_insts.sort_by(|a, b| b.1.cmp(a.1));

    println!("Top 10 most executed instructions:");
    for (i, (inst, count)) in sorted_insts.iter().take(10).enumerate() {
        #[expect(clippy::cast_precision_loss)]
        let percentage = (**count as f64 / trace_data.len() as f64) * 100.0;
        println!(
            "  {}. {}: {} times ({:.1}%)",
            i + 1,
            inst,
            count,
            percentage
        );
    }

    // Summary
    println!("\n{}", "=".repeat(80));
    println!("\n📋 SUMMARY:");
    println!("  Total entries: {}", trace_data.len());
    println!("  Unique PCs: {}", pc_distribution.len());
    println!("  Unique instructions: {}", inst_distribution.len());
    println!("  Tight loops: {}", tight_loops.len());
    println!("  Loop patterns: {}", loop_patterns.len());

    if !tight_loops.is_empty() || !loop_patterns.is_empty() {
        println!("\n⚠️  Warning: Loops detected - test may be stuck or waiting!");
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct TraceData {
    pc: u16,
    instruction: String,
}

fn load_trace_data(path: &PathBuf) -> Result<Vec<TraceData>> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut data = Vec::new();
    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let json: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse line {line_num}"))?;

        if let (Some(pc), Some(inst)) = (
            json.get("pc").and_then(serde_json::Value::as_u64),
            json.get("instruction").and_then(|v| v.as_str()),
        ) {
            #[expect(clippy::cast_possible_truncation)]
            data.push(TraceData {
                pc: pc as u16,
                instruction: inst.to_string(),
            });
        }
    }

    Ok(data)
}

fn detect_tight_loops(trace_data: &[TraceData], threshold: usize) -> Vec<TightLoop> {
    let mut tight_loops = Vec::new();
    let mut current_pc: Option<u16> = None;
    let mut current_inst: Option<String> = None;
    let mut consecutive_count = 0;
    let mut start_line = 0;

    for (i, entry) in trace_data.iter().enumerate() {
        if Some(entry.pc) == current_pc && Some(&entry.instruction) == current_inst.as_ref() {
            consecutive_count += 1;
        } else {
            if consecutive_count >= threshold {
                tight_loops.push(TightLoop {
                    line: start_line,
                    pc: current_pc.unwrap(),
                    instruction: current_inst.unwrap(),
                    consecutive_count,
                });
            }
            current_pc = Some(entry.pc);
            current_inst = Some(entry.instruction.clone());
            consecutive_count = 1;
            start_line = i;
        }
    }

    // Check last sequence
    if consecutive_count >= threshold {
        tight_loops.push(TightLoop {
            line: start_line,
            pc: current_pc.unwrap(),
            instruction: current_inst.unwrap(),
            consecutive_count,
        });
    }

    tight_loops
}

fn detect_loop_patterns(trace_data: &[TraceData], min_iterations: usize) -> Vec<LoopPattern> {
    // Simple pattern detection: look for sequences that repeat
    // This is a simplified version - a full implementation would use more sophisticated pattern matching

    let mut patterns = Vec::new();

    // Try different pattern lengths (2-10 instructions)
    for pattern_len in 2..=10 {
        if pattern_len > trace_data.len() / 2 {
            break;
        }

        let mut i = 0;
        while i + pattern_len * 2 <= trace_data.len() {
            // Extract potential pattern
            let pattern: Vec<_> = trace_data[i..i + pattern_len]
                .iter()
                .map(|e| (e.pc, &e.instruction))
                .collect();

            // Count how many times it repeats
            let mut iterations = 1;
            let mut j = i + pattern_len;

            while j + pattern_len <= trace_data.len() {
                let next_segment: Vec<_> = trace_data[j..j + pattern_len]
                    .iter()
                    .map(|e| (e.pc, &e.instruction))
                    .collect();

                if pattern == next_segment {
                    iterations += 1;
                    j += pattern_len;
                } else {
                    break;
                }
            }

            if iterations >= min_iterations {
                patterns.push(LoopPattern {
                    start_line: i,
                    end_line: j,
                    instructions: pattern.iter().map(|(_, inst)| (*inst).clone()).collect(),
                    iteration_count: iterations,
                });

                // Skip past this pattern
                i = j;
            } else {
                i += 1;
            }
        }
    }

    patterns
}

fn analyze_pc_distribution(trace_data: &[TraceData]) -> HashMap<u16, usize> {
    let mut distribution = HashMap::new();
    for entry in trace_data {
        *distribution.entry(entry.pc).or_insert(0) += 1;
    }
    distribution
}

fn analyze_instruction_distribution(trace_data: &[TraceData]) -> HashMap<String, usize> {
    let mut distribution = HashMap::new();
    for entry in trace_data {
        *distribution.entry(entry.instruction.clone()).or_insert(0) += 1;
    }
    distribution
}
