//! Trace indexing for fast lookups
//!
//! This module provides indexing capabilities for trace files,
//! enabling fast searches and queries without loading entire traces.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A range of line numbers in the trace file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRange {
    /// Starting line number (inclusive, 0-based)
    pub start: usize,
    /// Ending line number (exclusive)
    pub end: usize,
}

/// Index entry for a PC range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcRangeIndex {
    /// Program counter value
    pub pc: u16,
    /// Line ranges where this PC appears
    pub line_ranges: Vec<LineRange>,
    /// Total occurrences
    pub count: usize,
}

/// Index entry for an instruction type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionIndex {
    /// Instruction mnemonic (e.g., "LD A, B", "JR NZ, $FD")
    pub instruction: String,
    /// Line ranges where this instruction appears
    pub line_ranges: Vec<LineRange>,
    /// Total occurrences
    pub count: usize,
}

/// Index entry for register state checkpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterCheckpoint {
    /// Line number in the trace file
    pub line: usize,
    /// Program counter at this checkpoint
    pub pc: u16,
    /// Register values at this checkpoint
    pub registers: RegisterState,
}

/// Register state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterState {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
}

/// Memory access pattern tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAccessPattern {
    /// Memory address
    pub address: u16,
    /// Read count
    pub reads: usize,
    /// Write count
    pub writes: usize,
    /// Line numbers where this address is accessed
    pub access_lines: Vec<usize>,
}

/// Complete trace index for fast lookups
#[derive(Debug, Serialize, Deserialize)]
pub struct TraceIndex {
    /// Index version for compatibility
    pub version: String,
    /// Source trace file name
    pub source_file: String,
    /// Total number of entries in the trace
    pub total_entries: usize,
    /// PC range index for quick navigation
    pub pc_index: HashMap<u16, PcRangeIndex>,
    /// Instruction type index
    pub instruction_index: HashMap<String, InstructionIndex>,
    /// Register state checkpoints (every N instructions)
    pub checkpoints: Vec<RegisterCheckpoint>,
    /// Memory access patterns
    pub memory_access: HashMap<u16, MemoryAccessPattern>,
    /// Checkpoint interval (how often checkpoints are taken)
    pub checkpoint_interval: usize,
}

impl TraceIndex {
    /// Create a new empty trace index
    #[must_use]
    pub fn new(source_file: String, checkpoint_interval: usize) -> Self {
        Self {
            version: "1.0".to_string(),
            source_file,
            total_entries: 0,
            pc_index: HashMap::new(),
            instruction_index: HashMap::new(),
            checkpoints: Vec::new(),
            memory_access: HashMap::new(),
            checkpoint_interval,
        }
    }

    /// Build index from a JSONL trace file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    #[expect(clippy::too_many_lines)]
    pub fn build_from_jsonl(
        trace_path: &Path,
        checkpoint_interval: usize,
    ) -> std::io::Result<Self> {
        use std::io::{BufRead, BufReader};

        let file = std::fs::File::open(trace_path)?;
        let reader = BufReader::new(file);

        let source_file = trace_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut index = Self::new(source_file, checkpoint_interval);
        let mut line_num = 0;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            // Parse the JSON line
            let entry: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            // Extract fields
            if let Some(pc) = entry.get("pc").and_then(serde_json::Value::as_u64) {
                #[expect(clippy::cast_possible_truncation)]
                let pc = pc as u16;

                // Update PC index
                index
                    .pc_index
                    .entry(pc)
                    .and_modify(|idx| {
                        idx.count += 1;
                        // Merge adjacent line ranges
                        if let Some(last_range) = idx.line_ranges.last_mut() {
                            if last_range.end == line_num {
                                last_range.end = line_num + 1;
                            } else {
                                idx.line_ranges.push(LineRange {
                                    start: line_num,
                                    end: line_num + 1,
                                });
                            }
                        } else {
                            idx.line_ranges.push(LineRange {
                                start: line_num,
                                end: line_num + 1,
                            });
                        }
                    })
                    .or_insert_with(|| PcRangeIndex {
                        pc,
                        line_ranges: vec![LineRange {
                            start: line_num,
                            end: line_num + 1,
                        }],
                        count: 1,
                    });
            }

            // Update instruction index
            if let Some(instruction) = entry.get("instruction").and_then(|v| v.as_str()) {
                let instruction = instruction.to_string();

                index
                    .instruction_index
                    .entry(instruction.clone())
                    .and_modify(|idx| {
                        idx.count += 1;
                        // Merge adjacent line ranges
                        if let Some(last_range) = idx.line_ranges.last_mut() {
                            if last_range.end == line_num {
                                last_range.end = line_num + 1;
                            } else {
                                idx.line_ranges.push(LineRange {
                                    start: line_num,
                                    end: line_num + 1,
                                });
                            }
                        } else {
                            idx.line_ranges.push(LineRange {
                                start: line_num,
                                end: line_num + 1,
                            });
                        }
                    })
                    .or_insert_with(|| InstructionIndex {
                        instruction,
                        line_ranges: vec![LineRange {
                            start: line_num,
                            end: line_num + 1,
                        }],
                        count: 1,
                    });
            }

            // Create checkpoints at regular intervals
            if line_num % checkpoint_interval == 0
                && let Some(pc) = entry.get("pc").and_then(serde_json::Value::as_u64)
            {
                #[expect(clippy::cast_possible_truncation)]
                let registers = RegisterState {
                    a: entry
                        .get("a")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u8,
                    f: entry
                        .get("f")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u8,
                    b: entry
                        .get("b")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u8,
                    c: entry
                        .get("c")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u8,
                    d: entry
                        .get("d")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u8,
                    e: entry
                        .get("e")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u8,
                    h: entry
                        .get("h")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u8,
                    l: entry
                        .get("l")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u8,
                    sp: entry
                        .get("sp")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as u16,
                };

                #[expect(clippy::cast_possible_truncation)]
                index.checkpoints.push(RegisterCheckpoint {
                    line: line_num,
                    pc: pc as u16,
                    registers,
                });
            }

            line_num += 1;
        }

        index.total_entries = line_num;
        Ok(index)
    }

    /// Export index to a JSON file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn export(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load index from a JSON file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let index = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(index)
    }

    /// Find line ranges for a specific PC value
    #[must_use]
    pub fn find_pc(&self, pc: u16) -> Option<&PcRangeIndex> {
        self.pc_index.get(&pc)
    }

    /// Find line ranges for a specific instruction
    #[must_use]
    pub fn find_instruction(&self, instruction: &str) -> Option<&InstructionIndex> {
        self.instruction_index.get(instruction)
    }

    /// Find the nearest checkpoint before a given line
    #[must_use]
    pub fn find_checkpoint_before(&self, line: usize) -> Option<&RegisterCheckpoint> {
        self.checkpoints
            .iter()
            .rev()
            .find(|checkpoint| checkpoint.line <= line)
    }

    /// Get statistics about the trace
    #[must_use]
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            total_entries: self.total_entries,
            unique_pcs: self.pc_index.len(),
            unique_instructions: self.instruction_index.len(),
            checkpoint_count: self.checkpoints.len(),
            memory_addresses: self.memory_access.len(),
        }
    }
}

/// Statistics about the trace index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    /// Total trace entries
    pub total_entries: usize,
    /// Number of unique PC values
    pub unique_pcs: usize,
    /// Number of unique instructions
    pub unique_instructions: usize,
    /// Number of checkpoints
    pub checkpoint_count: usize,
    /// Number of memory addresses accessed
    pub memory_addresses: usize,
}
