use super::*;

/// Python async iteration/context protocol boundaries must stay distinct even
/// when both units are already inside an async function. This pins the
/// `async for`/`async with` source-protocol boundaries themselves rather than
/// relying on the outer `async def` boundary.
#[test]
fn query_mode_semantic_rejects_unproven_python_async_protocol_lifecycle_convergence() {
    let project = TempProject::new("py_async_protocol_lifecycle_boundary");
    project.write(
        "async_for.py",
        "async def first(xs):\n    async for x in xs:\n        return x\n    return None\n",
    );
    project.write(
        "sync_for.py",
        "async def first(xs):\n    for x in xs:\n        return x\n    return None\n",
    );
    project.write(
        "async_with.py",
        "async def use(cm):\n    async with cm:\n        return 1\n",
    );
    project.write(
        "sync_with.py",
        "async def use(cm):\n    with cm:\n        return 1\n",
    );

    let json = project.query_semantic_min_json();
    for pair in [
        ["async_for.py", "sync_for.py"],
        ["async_with.py", "sync_with.py"],
    ] {
        assert!(
            !family_contains_all(&json, &pair),
            "Python async lifecycle protocol boundary must not merge with the sync form for {pair:?}: {json}"
        );
    }
}

#[test]
fn query_mode_semantic_rejects_unproven_python_async_comprehension_convergence() {
    let project = TempProject::new("py_async_comprehension_boundary");
    project.write(
        "async_comp.py",
        "async def collect(xs):\n    return [x async for x in xs]\n",
    );
    project.write(
        "sync_comp.py",
        "async def collect(xs):\n    return [x for x in xs]\n",
    );

    let json = project.query_semantic_min_json();
    assert!(
        !family_contains_all(&json, &["async_comp.py", "sync_comp.py"]),
        "Python async comprehension protocol boundary must not merge with sync comprehension: {json}"
    );
}
