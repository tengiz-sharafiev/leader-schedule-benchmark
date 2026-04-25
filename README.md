# Leader Schedule Bench

Rust benchmarking project comparing two algorithms for finding the next eligible leader slot in a Solana epoch.

## Problem

Given a set of validators (selected by stake weight) and a current slot, find the next slot assigned to one of those validators. This is a hot-path operation that needs to be as fast as possible.

## Algorithms

### "Old" — Linear Scan

Builds a flat `Vec<Pubkey>` indexed by slot offset within the epoch (432,000 entries). Starting from the current slot, linearly scans forward until a matching validator is found.

- **Time complexity**: O(k), where k is the distance to the next matching slot
- **Space**: O(slots_per_epoch)
- Cache-friendly sequential memory access

### "New" — Binary Search

Stores per-validator sorted slot lists in a `HashMap<Pubkey, Vec<Slot>>`. For each validator in the target set, performs a `partition_point` (binary search) to find the next slot, then takes the global minimum.

- **Time complexity**: O(n * log(m)), where n = validator count, m = slots per validator
- **Space**: O(slots_per_epoch) total across all lists
- Random memory access across multiple vectors

## Benchmark Results

Benchmark parameters:
- Epoch 960 (Solana mainnet)
- Validator set: 50 validators, combined stake weight <= 10%
- 100 evenly-spaced probe slots across the epoch
- Criterion, 100 samples per algorithm

| Algorithm | Time (per 100 lookups) | Per lookup |
|-----------|----------------------|------------|
| **old** (linear scan) | **12.68 µs** | ~127 ns |
| **new** (binary search) | **76.45 µs** | ~765 ns |

The linear scan is **~6x faster** than the binary search approach. The flat `Vec<Pubkey>` benefits from sequential memory access and CPU cache prefetching, which outweighs the theoretical advantage of binary search when the average scan distance is short (with 50 validators covering ~10% of slots, the expected gap is ~10 slots).

## Usage

### Prerequisites

Create a `.env` file with a Solana mainnet RPC endpoint:

```
SOLANA_RPC_URL=https://api.mainnet-beta.solana.com
```

### Build & Test

```bash
cargo build          # Build the library
cargo test           # Run tests
cargo bench          # Run all benchmarks
```

## Dependencies

- `solana-rpc-client` — async Solana RPC client
- `solana-pubkey` / `solana-clock` / `solana-epoch-schedule` — Solana types
- `hashbrown` — fast HashMap/HashSet
- `itertools` — iterator utilities
- `criterion` — benchmarking framework
