## Semantic regression smoke

**Status:** `pass`

| Signal | Baseline | Current | Adjusted delta | Result |
| --- | ---: | ---: | ---: | --- |
| `aggregate` | 2222.28 ms | 2105.79 ms | -100.91 ms / -4.54% | within threshold |
| `curl` | 361.43 ms | 341.07 ms | -22.51 ms / -6.23% | within threshold |
| `nushell` | 850.74 ms | 831.02 ms | -12.78 ms / -1.50% | within threshold |
| `prometheus` | 585.60 ms | 535.19 ms | -42.00 ms / -7.17% | within threshold |
| `rubocop` | 424.51 ms | 398.50 ms | -23.62 ms / -5.56% | within threshold |

Initial material signal confirmed with a focused rerun of: `curl`, `nushell`, `prometheus`, `rubocop`.

Output drift: 7 declared, 0 unexpected.

Base `2a7047bfa4533dc1a83f245df13eb9e0a922ffea` → head `653690b9792bb22ea9b9b642e161dde4be698d73`.
