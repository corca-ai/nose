## Semantic regression smoke

**Status:** `pass`

| Signal | Baseline | Current | Adjusted delta | Result |
| --- | ---: | ---: | ---: | --- |
| `aggregate` | 1450.52 ms | 1423.90 ms | -30.58 ms / -2.11% | within threshold |
| `curl` | 230.25 ms | 225.79 ms | -9.04 ms / -3.93% | within threshold |
| `netty` | 849.63 ms | 828.81 ms | -12.83 ms / -1.51% | within threshold |
| `prometheus` | 370.64 ms | 369.31 ms | -8.70 ms / -2.35% | within threshold |

Initial material signal confirmed with a focused rerun of: `curl`, `netty`, `prometheus`.

Output drift: 1 declared, 0 unexpected.

Base `2a7047bfa4533dc1a83f245df13eb9e0a922ffea` → head `653690b9792bb22ea9b9b642e161dde4be698d73`.
