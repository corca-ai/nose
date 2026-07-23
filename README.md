# nose

**nose** finds duplicated code — literal copy-paste, renamed copies, and the same
logic written in a different style or language — across Python, JavaScript,
TypeScript, Go, Rust, Java, C, Ruby, Swift, plus declarative **CSS** (computed-style
equivalence) and **HTML markup** (rendered-DOM equivalence), including the
`<script>`/`<style>`/markup regions inside Vue, Svelte, and HTML. It **proves** its
semantic matches are real — equal fingerprint ⟹ equal behavior, never a false
equivalence — and ranks candidates by refactoring value for triage or CI. One
self-contained Rust binary; no runtime, services, or network.

## What semantic means

Other copy-paste detectors compare token runs. `nose` lowers code into a
normalized semantic IL, so it can catch repeated computation even when the code
does not look copied:

```java
import java.util.Arrays;

class PairTotals {
    static int nested(int[] xs, int[] ys) {
        int total = 0;
        for (int x : xs) {
            for (int y : ys) {
                total += x + y;
            }
        }
        return total;
    }

    static int streamed(int[] xs, int[] ys) {
        return Arrays.stream(xs)
            .flatMap(x -> Arrays.stream(ys).map(y -> x + y))
            .reduce(0, (total, value) -> total + value);
    }
}
```

The methods look unrelated as text, but both visit the same ordered pairs and
sum `x + y` from zero. nose reports them as one exact family only when it can
prove the stream source, flattening, reduction seed and step, and callback
effects. Change one of those and the methods stay separate. See
[clone types](docs/clone-types.md) for the distinction between verified and
similarity-based findings.

## Install

```sh
# Homebrew (macOS / Linux):
brew install corca-ai/tap/nose

# Or the install script (downloads a prebuilt binary):
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/corca-ai/nose/releases/latest/download/nose-cli-installer.sh | sh
```

## Quick start

Point `nose query` at a directory to inspect its duplication. It prints a summary and
runnable next commands, so you can filter, group, or open one family without memorizing
flags:

```
$ nose query src
nose finds duplication in code and docs.
nose finds; you judge. Filter, group, sort, or open families to explore.
analyzed 23 files · python 14 · typescript 9

4 duplicated-code families.
  verified 1 (exact 1 · shared-core 0) · copy-paste 1 · similar 2
  verified = machine-checked evidence · exact = same unit behavior · shared-core = shared computation

best candidates:
  src/loaders/users.py:1  load_users  2 copies · 8/10 shared, 2p · ~8 removable · similar   nose query src id=b221962180
  …
next commands — replace <path> with your path; terms combine with AND:
  filter  nose query <path> witness=exact   keep only the exact-behavior families
  group   nose query <path> group=dir       totals by directory (or: witness, lang, scope)
  open    nose query <path> id=<id> full     one family: every copy + the extraction skeleton
```

`nose query` is the **one command** for every workflow over that dataset. Use it
broadly for agent/human exploration, and pin it tightly when it is a CI gate:

```sh
# Explore and triage
nose query src                               # summary, best candidates first
nose query src id=<fam> full                 # open one family: every copy + extraction skeleton
nose query src base=origin/main              # PR check: a change applied to one copy but not its siblings
nose query src --format markdown             # a ranked report to paste into a PR or issue
nose query src --format json                 # the versioned machine-readable contract
nose query docs                              # also reports same-language near-duplicate Markdown prose

# Gate on copy-paste only, with an explicit size budget
nose query src --mode syntax --min-size 80 'lines>25' --fail-on any
nose query src --mode syntax --min-size 80 'shared>20' --fail-on any
nose query src --mode syntax --min-size 80 'dup>80' --fail-on any
```

By default it runs all three channels — `syntax` (copy-paste runs), `semantic` (exact
same-logic Type-4 clones), and `near` (fuzzy near-duplicates) — and respects `.gitignore`;
pass `--mode` to run exactly the channels you list. See
[agent-recipe](docs/agent-recipe.md) for the exploration loop and
[continuous integration](docs/continuous-integration.md) for stable copy-paste-only gates,
baselines, and SARIF.

## Faster local feedback

Reuse unchanged analysis when you rerun a query, or keep a foreground JSONL
session open for an editor or local tool:

```sh
nose query . --cache-dir .nose-cache
nose query . --cache-dir .nose-cache --watch --format jsonl
```

Caching is explicit and does not change the findings. Watch mode starts no
daemon or network service. See [query cache](docs/query-cache.md) and
[watch mode](docs/query-watch.md).

## Documentation

- [Getting started](docs/getting-started.md) — your first `nose query` and how to read the report.
- [Documentation home](docs/home.md) — a guided path from first run to team adoption.
- [Query cache](docs/query-cache.md) — faster repeated runs and safe cache cleanup.
- [Watch mode](docs/query-watch.md) — live snapshots for editors and local tools.
- [Usage](docs/usage.md) — command and flag reference.
- [Configuration](docs/configuration.md) — `nose.toml`, excludes, modes, thresholds, ignores.
- [Contributing](CONTRIBUTING.md) — development workflow and quality gates.
- [Agent instructions](AGENTS.md) — repository-specific instructions for coding agents.

## Status

Pre-1.0. Current capability, language, JSON, benchmark, and soundness details live in the
wiki so claims have one source of truth:

- [Languages](docs/languages.md)
- [Markdown duplication](docs/markdown-duplication.md)
- [Clone types](docs/clone-types.md)
- [Benchmark](docs/benchmark.md)
- [Query JSON contract](docs/query-json.md)

## License

MIT
