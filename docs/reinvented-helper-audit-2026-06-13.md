# Reinvented-helper field audit — 2026-06-13

A hand-labeled audit of the [reinvented-helper containment channel](reinvented-helpers.md)
on the full `bench/repos` corpus (105 repos), the **measured-precision instrument** design
§2c requires before a finding class enters the bare-default surface. It is the basis for
promoting the channel from a one-line count to a listed default section.

## Method

`nose scan <repo> --format json` over every corpus repo; every `reinvented_helpers`
finding collected and hand-labeled by reading the helper body and the container's matched
region. A finding is **actionable** when the container genuinely recomputes the helper's
value AND replacing it with a call is a valid fix; a **value-duplication** when the two
genuinely compute the same value but the fix needs consumer judgment (cross-type
adaptation, a class-override relationship); **noise** when the "reinvention" is wrong,
circular, or intentional.

## Result — 17 findings, 9 repos

| precision lens | count | rate |
|---|---|---|
| genuine value-duplication (the channel's exactness claim) | 16 / 17 | 94% |
| directly actionable (call the helper as-is) | 10 / 17 | 59% |
| directly actionable, **non-test code** | 10 / 14 | 71% |

The single non-value-duplication ([15], jsoup `ensureAttributeCapacity` ⟵ `ensureCapacity`)
is a weak, approximate-site match whose bodies actually differ — it did not reproduce in
isolation (cross-file context artifact). All others are genuine value matches.

### Actionable (10) — call the helper directly
- **raylib** `CheckCollisionPointCircle` ⟵ `Vector2DistanceSqr`; `QuaternionInvert` ⟵ `Vector4LengthSqr` (Quaternion is a Vector4 typedef).
- **libgdx** `quadrilateralCentroid`'s `area1` ⟵ `triangleArea`; `nearestSegmentPoint`'s `length2` ⟵ `Vector2.dst2`; `argb8888` ⟵ `rgb888`; `toFloatBits`'s `color` ⟵ `toIntBits`.
- **prettier** `isAsConstExpression`'s second disjunct ⟵ `isTsAsConstExpression`.
- **sympy** `_print_contents`'s else-branch ⟵ `_print`.
- **delve** `isBadNum` ⟵ `isNum` (`isNum(v) && len(v) > 1 && v[0] == '0'`).
- **h2database** `getGarbageCollectionCount` ⟵ `getGarbageCollectionTime` — and this one is a **real upstream bug**: the count loop copy-pasted `getCollectionTime()` (should be `getCollectionCount()`), which is *why* it recomputes the time helper's value.

### Value-duplication, consumer-judgment (4)
- **raylib** `Vector4DistanceSqr` / `Vector4DotProduct` contain the 2D / 3D subcomputation of `Vector2DistanceSqr` / a vendored `ma_vec3f_dot` — genuine, but the helper's parameter type differs (the value model erases types; the call needs adaptation).
- **sqlalchemy** `visit_mod_binary` (base compiler) ⟵ the pg8000 dialect override — same code path, but a base cannot call a subclass override.

### Noise (3) — all TEST code or weak
- **sympy** `test_type_G` asserts `positive_roots()`'s value as a dict literal — calling the helper would make the test circular.
- **prettier** `bar2` ⟵ `bar` — intentional near-identical flow-test inputs.
- **poetry** `test_python_installer_install` inlines the `mock_get_download_link` fixture — a genuine test DRY, but test scaffolding (judgment-deep, §2b).

## Decision

The non-actionable findings are **dominated by test-container code** (the `test_type_G`
circular assertion, the prettier test inputs, the poetry fixture). This is a *decidable*
class (§2b): **test-container findings are excluded from the bare-default surface** (kept
in `--show reinvented` and the JSON, with `container_in_test: true`). After that filter
the default surface is 14 findings, **94% genuine value-duplications and 71% directly
actionable** — dominated by actionable findings, clearing the §2c bar. The channel is
therefore **promoted**: the default human report lists the (non-test) findings instead of
a count.

The cross-type value-duplications stay on the surface as genuine evidence — per the
consumer model (§2), the calling agent prices the type adaptation cheaply; nose carries
the witness, not the verdict. The [docs caveat](reinvented-helpers.md) on type-checking
the suggested call stands.

*See also: [reinvented-helpers](reinvented-helpers.md) · [design §2c](design.md) ·
[field-evaluation](field-evaluation.md).*
