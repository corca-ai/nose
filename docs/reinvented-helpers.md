# Reinvented helpers

Sometimes a codebase already has a helper, but another function repeats that
helper's computation inline. nose calls this a **reinvented helper** and suggests
reusing the existing helper.

These findings are different from ordinary clone families: the useful
relationship is “this larger function contains the helper's computation,” not
“these two functions are the same size.”

## Find them

Non-test findings appear in the default report. Use the dedicated view to see
the complete ranked list:

```sh
nose query . reinvented
```

Each item identifies:

- the existing `helper`;
- the `site` that repeats its computation;
- an estimated refactoring value; and
- whether the source location is `approximate`.

The suggested direction is to call the helper for the repeated part. It is not
an automatic rewrite.

## Review a finding

Before changing code, check:

1. **Can the site call the helper?** The computation may match even when nominal
   types, visibility, ownership, or module boundaries make the call unsuitable.
2. **Is the site approximate?** A synthesized loop or fold may only map back to
   the surrounding function. Replace the matching sub-computation, not the whole
   reported range.
3. **Is the helper in production code?** Test-only helpers are not suggested for
   production callers. Test and vendored sites may still be intentional.
4. **Would a call be clearer?** A verified repeated computation can still be too
   small or context-specific to justify a refactor.

nose excludes ordinary callers: code that already calls the helper is the
desired outcome, not a finding.

## Machine output and limits

The query JSON `reinvented` view contains `items[]` with the helper, site, value,
and `approximate` fields. See [query JSON](query-json.md#views) for the contract.

The match verifies the modeled computation, but it does not promise that a
mechanical replacement compiles or preserves surrounding behavior. This is why
the report supplies evidence and a direction while leaving the refactoring
decision to the user.

The exact containment contract, size floors, exclusions, and field measurements
are in the [implementation and evidence reference](reinvented-helpers-internals.md).
