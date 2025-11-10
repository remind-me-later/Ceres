#!/usr/bin/env python3
"""
Trace Analysis Tool for Ceres Game Boy Emulator

This script analyzes execution traces exported from the test runner,
providing various insights into emulator behavior for debugging purposes.

Usage:
    python analyze_trace.py <trace_file.json> [--last N] [--inst INSTRUCTION] [--range START END]

Examples:
    # Show last 20 instructions
    python analyze_trace.py target/traces/1234567890_trace.json --last 20

    # Find all JP instructions
    python analyze_trace.py target/traces/1234567890_trace.json --inst JP

    # Show instructions in PC range 0x0150-0x0160
    python analyze_trace.py target/traces/1234567890_trace.json --range 0x0150 0x0160

    # Generate instruction histogram
    python analyze_trace.py target/traces/1234567890_trace.json --histogram
"""

import argparse
import json
import sys
from collections import Counter
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional


def load_trace(filepath: Path) -> Dict:
    """Load trace JSON file."""
    try:
        with open(filepath, 'r') as f:
            return json.load(f)
    except FileNotFoundError:
        print(f"Error: Trace file not found: {filepath}", file=sys.stderr)
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON in trace file: {e}", file=sys.stderr)
        sys.exit(1)


def format_registers(regs: Dict) -> str:
    """Format register state for display."""
    return (f"A={regs['a']:02X} F={regs['f']:02X} "
            f"BC={regs['b']:02X}{regs['c']:02X} "
            f"DE={regs['d']:02X}{regs['e']:02X} "
            f"HL={regs['h']:02X}{regs['l']:02X} "
            f"SP={regs['sp']:04X}")


def print_entry(entry: Dict, show_registers: bool = True):
    """Print a single trace entry."""
    pc = entry['pc']
    inst = entry['instruction']
    cycles = entry['cycles']
    
    print(f"[{pc:04X}] {inst:<20} (cycles: {cycles})", end='')
    
    if show_registers:
        print(f" ; {format_registers(entry['registers'])}")
    else:
        print()


def show_last_n(trace: Dict, n: int, show_registers: bool = True):
    """Show the last N instructions."""
    entries = trace['entries']
    count = min(n, len(entries))
    
    print(f"\n=== Last {count} Instructions ===\n")
    for entry in entries[-count:]:
        print_entry(entry, show_registers)


def find_instruction(trace: Dict, mnemonic: str, show_registers: bool = True):
    """Find all occurrences of an instruction mnemonic."""
    entries = [e for e in trace['entries'] 
               if mnemonic.upper() in e['instruction'].upper()]
    
    print(f"\n=== Found {len(entries)} occurrences of '{mnemonic}' ===\n")
    for entry in entries:
        print_entry(entry, show_registers)


def show_range(trace: Dict, start_pc: int, end_pc: int, show_registers: bool = True):
    """Show instructions in a PC range."""
    entries = [e for e in trace['entries'] 
               if start_pc <= e['pc'] <= end_pc]
    
    print(f"\n=== Instructions in range {start_pc:04X}-{end_pc:04X} ({len(entries)} entries) ===\n")
    for entry in entries:
        print_entry(entry, show_registers)


def show_histogram(trace: Dict, top_n: int = 20):
    """Show instruction frequency histogram."""
    # Extract instruction mnemonics (without operands for grouping)
    instructions = []
    for entry in trace['entries']:
        # Get mnemonic (first word before space or comma)
        mnemonic = entry['instruction'].split()[0].split(',')[0]
        instructions.append(mnemonic)
    
    counter = Counter(instructions)
    total = len(instructions)
    
    print(f"\n=== Instruction Frequency (Top {top_n}) ===\n")
    print(f"{'Instruction':<15} {'Count':>8} {'Percentage':>10}")
    print("-" * 40)
    
    for inst, count in counter.most_common(top_n):
        percentage = (count / total) * 100
        print(f"{inst:<15} {count:>8} {percentage:>9.2f}%")
    
    print(f"\nTotal instructions: {total}")


def detect_loops(trace: Dict, min_iterations: int = 3):
    """Detect potential infinite loops by finding repeated PC sequences."""
    entries = trace['entries']
    if len(entries) < 10:
        return
    
    print(f"\n=== Potential Loops (min {min_iterations} iterations) ===\n")
    
    # Track PC sequences
    window_size = 5
    sequences: Dict[tuple, List[int]] = {}
    
    for i in range(len(entries) - window_size):
        seq = tuple(e['pc'] for e in entries[i:i+window_size])
        if seq not in sequences:
            sequences[seq] = []
        sequences[seq].append(i)
    
    # Find sequences that repeat
    found_loops = False
    for seq, positions in sequences.items():
        if len(positions) >= min_iterations:
            found_loops = True
            print(f"Loop detected at positions {positions[:5]}{'...' if len(positions) > 5 else ''}")
            print(f"  PC sequence: {' -> '.join(f'{pc:04X}' for pc in seq)}")
            print(f"  Iterations: {len(positions)}")
            print()
    
    if not found_loops:
        print("No loops detected")


def show_metadata(trace: Dict):
    """Display trace metadata."""
    meta = trace['metadata']
    timestamp = datetime.fromtimestamp(meta['timestamp'])
    
    print("\n=== Trace Metadata ===\n")
    print(f"Timestamp:        {timestamp.strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"Entry count:      {meta['entry_count']}")
    print(f"Buffer capacity:  {meta['buffer_capacity']}")
    print(f"Buffer usage:     {(meta['entry_count']/meta['buffer_capacity']*100):.1f}%")


def main():
    parser = argparse.ArgumentParser(
        description='Analyze Ceres emulator execution traces',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument('trace_file', type=Path, help='Path to trace JSON file')
    parser.add_argument('--last', type=int, metavar='N', 
                       help='Show last N instructions')
    parser.add_argument('--inst', metavar='INSTRUCTION', 
                       help='Find specific instruction mnemonic')
    parser.add_argument('--range', nargs=2, metavar=('START', 'END'),
                       help='Show instructions in PC range (hex values)')
    parser.add_argument('--histogram', action='store_true',
                       help='Show instruction frequency histogram')
    parser.add_argument('--loops', action='store_true',
                       help='Detect potential infinite loops')
    parser.add_argument('--no-registers', action='store_true',
                       help='Hide register state in output')
    parser.add_argument('--top', type=int, default=20, metavar='N',
                       help='Number of entries to show in histogram (default: 20)')
    
    args = parser.parse_args()
    
    # Load trace
    trace = load_trace(args.trace_file)
    show_registers = not args.no_registers
    
    # Show metadata first
    show_metadata(trace)
    
    # Process commands
    if args.last:
        show_last_n(trace, args.last, show_registers)
    
    if args.inst:
        find_instruction(trace, args.inst, show_registers)
    
    if args.range:
        try:
            start = int(args.range[0], 16)
            end = int(args.range[1], 16)
            show_range(trace, start, end, show_registers)
        except ValueError:
            print("Error: Range values must be hex numbers (e.g., 0x0150)", file=sys.stderr)
            sys.exit(1)
    
    if args.histogram:
        show_histogram(trace, args.top)
    
    if args.loops:
        detect_loops(trace)
    
    # If no specific command, show summary
    if not (args.last or args.inst or args.range or args.histogram or args.loops):
        print("\nNo analysis command specified. Use --help for options.")
        print("Quick start:")
        print(f"  python {sys.argv[0]} {args.trace_file} --last 20")
        print(f"  python {sys.argv[0]} {args.trace_file} --histogram")


if __name__ == '__main__':
    main()
