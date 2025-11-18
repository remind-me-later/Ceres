# Quick Reference: SQL Queries

Quick lookup table for which query to use for different debugging scenarios.

## I want to...

### Performance Issues

- **Find infinite loops or stuck code** → `tight_loops.sql`
- **See what code runs most often** → `instruction_hotspots.sql`
- **Measure frame timing** → `frame_timing.sql`
- **Find performance bottlenecks** → `instruction_hotspots.sql`

### Graphics/PPU Debugging

- **Check PPU timing per scanline** → `ppu_mode_timeline.sql`
- **See when sprites are uploaded** → `dma_transfers.sql`
- **Find which tiles are being updated** → `memory_hotspots.sql`

### Behavior Analysis

- **Track data through registers** → `register_changes.sql`
- **Compare execution between runs** → `execution_fingerprint.sql`
- **See DMA activity** → `dma_transfers.sql`

### Memory Debugging

- **Find hot memory addresses** → `memory_hotspots.sql`
- **Track VRAM/OAM writes** → `memory_hotspots.sql`
- **See DMA uploads** → `dma_transfers.sql`

## Query Complexity

| Query                       | Result Size           | Speed  | Use Case               |
| --------------------------- | --------------------- | ------ | ---------------------- |
| `tight_loops.sql`           | Small (10-50 rows)    | Fast   | Quick loop detection   |
| `instruction_hotspots.sql`  | Medium (50-100 rows)  | Fast   | Performance profiling  |
| `frame_timing.sql`          | Small (frames only)   | Fast   | Frame rate analysis    |
| `ppu_mode_timeline.sql`     | Large (1000s)         | Medium | Detailed PPU debugging |
| `dma_transfers.sql`         | Small (10-100 rows)   | Fast   | DMA activity tracking  |
| `memory_hotspots.sql`       | Medium (100-500 rows) | Medium | Memory access patterns |
| `register_changes.sql`      | Large (1000s)         | Slow   | Detailed debugging     |
| `execution_fingerprint.sql` | Medium (100s)         | Fast   | Regression testing     |

## Tips

- Start with fast queries to get overview
- Use Perfetto's time range selector to narrow results
- Export query results to CSV for deeper analysis
- Combine multiple queries to cross-reference findings
