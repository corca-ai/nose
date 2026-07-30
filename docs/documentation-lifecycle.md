# Documentation lifecycle

The nose wiki separates current operating guidance from durable historical
evidence. The checked [lifecycle catalog](lifecycle.json) classifies every
top-level Markdown page without relying on front matter that older supported
versions of `awiki` do not understand.

## Page kinds

| Kind | Purpose | Update rule |
|---|---|---|
| `guide` | A task-oriented path for users or contributors | Verify commands and outcomes against the current product before the review window expires. |
| `reference` | Current interfaces, architecture, contracts, or operating facts | Keep exact names, versions, and links synchronized with their implementation owner. |
| `decision` | A maintained design constraint and its rationale | Amend the decision when the constraint changes; preserve rejected alternatives when they still prevent repeated work. |
| `active-roadmap` | Open sequencing, pricing, or migration work | Recheck frequently, close completed work, and move durable conclusions into a decision or historical record. |
| `historical-record` | A dated measurement, audit, release record, or append-only ledger | Preserve claims and reproduction context. Point readers to current guidance instead of rewriting the old result. |

`active` pages are current contracts. `historical` pages are retained evidence,
not current product documentation. A `superseded` historical page must name a
replacement in the catalog.

## Ownership and freshness

Catalog collections assign an owner, `last_verified` date, and review window to
active pages. The owner is responsible for checking commands, tool versions,
schemas, and current-behavior claims in that collection. Historical collections
have an evidence owner and append-only retention, but no recurring freshness
window because their claims are bound to the recorded run.

The docs gate fails when:

- the exact Markdown inventory changes without a catalog refresh;
- an active page has no owner, verification date, or positive review window;
- an active collection is past its review window;
- a page appears in more than one explicit collection;
- historical or superseded metadata is incomplete; or
- the catalog names a page or replacement that does not exist.

The default rule classifies otherwise-unlisted Markdown anywhere below `docs/`
as maintained reference material owned by repository maintainers. The recursive
inventory digest still makes every added, removed, or renamed page an explicit
review event, so the default cannot silently absorb a new page.

## Navigation policy

The [documentation home](home.md) routes directly to current user guidance.
Maintainer-facing current contracts and active plans live under the
[development and evidence index](development-and-evidence.md). Dated audits,
closed issue records, release evidence, and the large append-only experiment and
dogfooding ledgers are routed through the [historical records
index](historical-records.md), which explains their retained context.

Large current references remain focused owner documents even when they are long.
Mixed pages should expose a stable current section first and route their
append-only material from the historical index. Split or rename a record only
when its stable anchors can be preserved or a checked migration is added.

## Updating the catalog

1. Choose the narrowest kind that describes the page.
2. Add it to an explicit collection when its owner or lifecycle differs from
   the maintained-reference default.
3. Recompute the inventory digest reported by the validator.
4. Run:

   ```sh
   python3 scripts/check-doc-lifecycle.py --selftest
   python3 scripts/check-doc-lifecycle.py
   ./scripts/check-docs.sh
   ```

Do not refresh `last_verified` mechanically. It means the collection's current
claims were actually checked by its named owner.
