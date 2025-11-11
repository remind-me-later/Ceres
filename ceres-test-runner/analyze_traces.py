#!/usr/bin/env python3
"""
Trace Analysis Tools

This script provides common analysis functions for examining
execution trace files exported by the test runner.
"""

import argparse
import json
import sys
from collections import Counter, defaultdict
from typing import Dict, List, Any


def load_trace(trace_file: str) -> Dict[str, Any]:
    """Load a trace file."""
    with open(trace_file, 'r') as f:
        return json.load(f)


def show_summary(trace: Dict[str, Any]):
    """Show a summary of the trace."""
    metadata = trace.get('metadata', {})
    entries = trace.get('entries', [])
    
    print(f"Trace Summary:")
    print(f"  Total Entries: {len(entries)}")
    print(f"  Entry Count (metadata): {metadata.get('entry_count', 'N/A')}")
    print(f"  Timestamp: {metadata.get('timestamp', 'N/A')}")
    
    # Count instruction occurrences
    instruction_counts = Counter()
    for entry in entries:
        if 'fields' in entry and 'instruction' in entry['fields']:
            instruction_counts[entry['fields']['instruction']] += 1
    
    print(f"\nTop 10 Most Used Instructions:")
    for inst, count in instruction_counts.most_common(10):
        print(f"  {count:5d}x {inst}")


def find_address_range(trace: Dict[str, Any], start_addr: int, end_addr: int):
    """Find all entries within a specific address range."""
    entries = trace.get('entries', [])
    matching = []
    
    for entry in entries:
        if 'fields' in entry and 'pc' in entry['fields']:
            pc = entry['fields']['pc']
            if isinstance(pc, int) and start_addr <= pc <= end_addr:
                matching.append(entry)
    
    print(f"Entries in address range 0x{start_addr:04X}-0x{end_addr:04X}:")
    for entry in matching[:20]:  # Limit output
        fields = entry.get('fields', {})
        pc = fields.get('pc', 'Unknown')
        instruction = fields.get('instruction', 'Unknown')
        print(f"  0x{pc:04X}: {instruction}")
    
    if len(matching) > 20:
        print(f"  ... ({len(matching) - 20} more entries)")


def find_loops(trace: Dict[str, Any], min_repeats=5):
    """Find potential infinite loops by identifying repeated PC patterns."""
    entries = trace.get('entries', [])
    
    # Count PC occurrences
    pc_counts = Counter()
    for entry in entries:
        if 'fields' in entry and 'pc' in entry['fields']:
            pc = entry['fields']['pc']
            pc_counts[pc] += 1
    
    # Find PCs with high repetition counts
    loops = [(pc, count) for pc, count in pc_counts.items() if count >= min_repeats]
    loops.sort(key=lambda x: x[1], reverse=True)
    
    print(f"Potential loops (instructions executed >= {min_repeats} times):")
    for pc, count in loops[:10]:
        # Find the instruction at this PC
        instruction = "Unknown"
        for entry in entries:
            if 'fields' in entry and 'pc' in entry['fields'] and entry['fields']['pc'] == pc:
                instruction = entry['fields'].get('instruction', 'Unknown')
                break
        
        print(f"  0x{pc:04X}: {instruction} executed {count} times")


def analyze_register_changes(trace: Dict[str, Any], reg='a'):
    """Analyze changes in a specific register over time."""
    entries = trace.get('entries', [])
    changes = []
    
    for entry in entries:
        if 'fields' in entry:
            fields = entry['fields']
            if reg in fields:
                pc = fields.get('pc', 0)
                value = fields[reg]
                instruction = fields.get('instruction', 'Unknown')
                changes.append((pc, value, instruction))
    
    print(f"Register {reg.upper()} changes over time:")
    for pc, value, instruction in changes[:20]:
        print(f"  0x{pc:04X}: {reg.upper()}=${value:02X} ({instruction})")
    
    if len(changes) > 20:
        print(f"  ... ({len(changes) - 20} more changes)")


def main():
    parser = argparse.ArgumentParser(description='Analyze execution trace files')
    parser.add_argument('trace_file', help='Path to the trace JSON file')
    parser.add_argument('--summary', action='store_true', help='Show trace summary')
    parser.add_argument('--address-range', nargs=2, type=lambda x: int(x, 0), 
                       metavar=('START', 'END'),
                       help='Find instructions in address range (prefix with 0x for hex)')
    parser.add_argument('--find-loops', action='store_true',
                       help='Find potential infinite loops')
    parser.add_argument('--register', choices=['a', 'f', 'b', 'c', 'd', 'e', 'h', 'l'],
                       help='Analyze changes in specific register')
    
    args = parser.parse_args()
    
    try:
        trace = load_trace(args.trace_file)
        
        if args.summary or not any([args.address_range, args.find_loops, args.register]):
            show_summary(trace)
            print()
        
        if args.address_range:
            start, end = args.address_range
            find_address_range(trace, start, end)
            print()
        
        if args.find_loops:
            find_loops(trace)
            print()
        
        if args.register:
            analyze_register_changes(trace, args.register)
            print()
            
    except FileNotFoundError:
        print(f"Error: Trace file '{args.trace_file}' not found.")
        sys.exit(1)
    except json.JSONDecodeError:
        print(f"Error: Invalid JSON in trace file '{args.trace_file}'.")
        sys.exit(1)
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)


if __name__ == '__main__':
    main()