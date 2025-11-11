#!/bin/bash

# Utility script for searching and filtering trace files

if [ $# -lt 1 ]; then
    echo "Usage: $0 <trace-file> [search-term]"
    echo "  Without search-term: prints basic info about the trace"
    echo "  With search-term: searches for matching entries"
    exit 1
fi

TRACE_FILE="$1"
SEARCH_TERM="$2"

echo "=== Analyzing trace: $TRACE_FILE ==="

# Check if file exists
if [ ! -f "$TRACE_FILE" ]; then
    echo "Error: Trace file '$TRACE_FILE' not found."
    exit 1
fi

if [ -z "$SEARCH_TERM" ]; then
    # Just show basic info
    ENTRY_COUNT=$(jq '.entries | length' "$TRACE_FILE" 2>/dev/null)
    
    if [ $? -ne 0 ]; then
        echo "Error: Invalid JSON file format."
        exit 1
    fi
    
    echo "Total entries: $ENTRY_COUNT"
    echo ""
    echo "First 5 entries:"
    jq '.entries[0:5]' "$TRACE_FILE" | head -20
    echo ""
    echo "Last 5 entries:"
    TOTAL=$(jq '.entries | length' "$TRACE_FILE")
    START=$((TOTAL > 5 ? TOTAL - 5 : 0))
    jq --argjson start $START --argjson total $TOTAL '.entries[$start:.entries | length]' "$TRACE_FILE"
    echo ""
    echo "Common instructions:"
    jq -r '.entries[] | select(.name == "EXECUTE_INSTRUCTION") | .fields.instruction' "$TRACE_FILE" 2>/dev/null | sort | uniq -c | sort -nr | head -10
    
else
    # Search for the term
    echo "Searching for: $SEARCH_TERM"
    MATCHES=$(jq --arg search "$SEARCH_TERM" -r '
        .entries[] | 
        select(
            .name | test($search; "i") or 
            .target | test($search; "i") or
            (.fields | to_entries[] | .value | tostring | test($search; "i"))
        ) | 
        "PC: \(.fields.pc//\"N/A\") | Target: \(.target) | Name: \(.name) | Fields: \(.fields)"
    ' "$TRACE_FILE")
    
    if [ $? -eq 0 ]; then
        if [ -n "$MATCHES" ]; then
            echo "$MATCHES" | head -20
            TOTAL_MATCHES=$(echo "$MATCHES" | wc -l)
            if [ "$TOTAL_MATCHES" -gt 20 ]; then
                echo "... and $(($TOTAL_MATCHES - 20)) more matches"
            fi
        else
            echo "No matches found."
        fi
    else
        echo "Error during search operation."
        exit 1
    fi
fi