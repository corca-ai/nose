use super::*;

#[test]
fn js_ts_promise_continuation_channels_and_callbacks_stay_split() {
    let i = Interner::new();
    let fulfilled = "function f() {\n  return Promise.resolve(1).then(x => x + 1);\n}\n";
    let custom_receiver = "function f(p) {\n  return p.then(x => x + 1);\n}\n";
    let rejected_unrecovered = "function f() {\n  return Promise.reject(1);\n}\n";
    let throwing_handler =
        "function f() {\n  return Promise.resolve(1).then(x => { throw x; });\n}\n";
    let catch_recovery = "function f() {\n  return Promise.reject(1).catch(e => e + 1);\n}\n";
    let sync_payload = "function f() {\n  return 2;\n}\n";

    assert_ne!(
        value_fp(&i, fulfilled, Lang::TypeScript),
        value_fp(&i, custom_receiver, Lang::TypeScript),
        "a `.then` selector on an arbitrary receiver is not PromiseLike proof"
    );
    assert_ne!(
        value_fp(&i, fulfilled, Lang::TypeScript),
        value_fp(&i, rejected_unrecovered, Lang::TypeScript),
        "fulfilled continuations must not erase rejected Promise channels"
    );
    assert_ne!(
        value_fp(&i, fulfilled, Lang::TypeScript),
        value_fp(&i, throwing_handler, Lang::TypeScript),
        "handlers that throw produce rejection-channel obligations"
    );
    assert_ne!(
        value_fp(&i, catch_recovery, Lang::TypeScript),
        value_fp(&i, sync_payload, Lang::TypeScript),
        "Promise.catch recovery must preserve the Promise boundary"
    );
}

#[test]
fn js_ts_executor_timer_microtask_and_cancellation_lifecycles_stay_split() {
    let i = Interner::new();
    let reject_then_resolve =
        "function f() {\n  return new Promise((resolve, reject) => { reject(1); resolve(2); });\n}\n";
    let resolve_then_reject =
        "function f() {\n  return new Promise((resolve, reject) => { resolve(2); reject(1); });\n}\n";
    let executor_throw =
        "function f() {\n  return new Promise((resolve, reject) => { throw 1; });\n}\n";
    let explicit_reject = "function f() {\n  return Promise.reject(1);\n}\n";
    let timeout = "function f(cb) {\n  setTimeout(cb, 0);\n  return 0;\n}\n";
    let immediate = "function f(cb) {\n  setImmediate(cb);\n  return 0;\n}\n";
    let direct_callback = "function f(cb) {\n  cb();\n  return 0;\n}\n";
    let microtask = "function f(cb) {\n  queueMicrotask(cb);\n  return 0;\n}\n";
    let promise_continuation = "function f(cb) {\n  Promise.resolve().then(cb);\n  return 0;\n}\n";
    let cancel_timeout = "function f(handle) {\n  return clearTimeout(handle);\n}\n";
    let cancel_frame = "function f(handle) {\n  return cancelAnimationFrame(handle);\n}\n";
    let frame = "function f(cb) {\n  requestAnimationFrame(cb);\n  return 0;\n}\n";
    let abort_reject = "function f(reason) {\n  return AbortSignal.abort(reason);\n}\n";
    let promise_reject = "function f(reason) {\n  return Promise.reject(reason);\n}\n";

    assert_ne!(
        value_fp(&i, reject_then_resolve, Lang::TypeScript),
        value_fp(&i, resolve_then_reject, Lang::TypeScript),
        "new Promise multi-settlement ordering is observable and must stay closed"
    );
    assert_ne!(
        value_fp(&i, executor_throw, Lang::TypeScript),
        value_fp(&i, explicit_reject, Lang::TypeScript),
        "executor throw-to-rejection needs explicit executor timing proof before recovery"
    );
    assert_ne!(
        value_fp(&i, timeout, Lang::TypeScript),
        value_fp(&i, immediate, Lang::TypeScript),
        "setTimeout and setImmediate have different task scheduling contracts"
    );
    assert_ne!(
        value_fp(&i, timeout, Lang::TypeScript),
        value_fp(&i, direct_callback, Lang::TypeScript),
        "timer scheduling must not collapse into synchronous callback invocation"
    );
    assert_ne!(
        value_fp(&i, microtask, Lang::TypeScript),
        value_fp(&i, promise_continuation, Lang::TypeScript),
        "queueMicrotask and Promise continuations are adjacent but distinct scheduling surfaces"
    );
    assert_ne!(
        value_fp(&i, cancel_timeout, Lang::TypeScript),
        value_fp(&i, cancel_frame, Lang::TypeScript),
        "timer and animation-frame cancellation lifecycles must not collapse"
    );
    assert_ne!(
        value_fp(&i, frame, Lang::TypeScript),
        value_fp(&i, microtask, Lang::TypeScript),
        "animation-frame scheduling is not microtask scheduling"
    );
    assert_ne!(
        value_fp(&i, abort_reject, Lang::TypeScript),
        value_fp(&i, promise_reject, Lang::TypeScript),
        "AbortSignal cancellation/liveness is not Promise rejection recovery"
    );
}

#[test]
fn go_channel_select_goroutine_and_defer_protocols_stay_split() {
    let i = Interner::new();
    let receive_value = "package p\nfunc f(ch <-chan int) int { return <-ch }\n";
    let receive_status = "package p\nfunc f(ch <-chan int) bool { _, ok := <-ch; return ok }\n";
    let send = "package p\nfunc f(ch chan<- int, x int) { ch <- x }\n";
    let assign = "package p\nfunc f(ch chan<- int, x int) { y := x; _ = y }\n";
    let goroutine = "package p\nfunc f(x int) { go record(x) }\n";
    let direct_call = "package p\nfunc f(x int) { record(x) }\n";
    let deferred = "package p\nfunc f(x int) { defer record(x); record(x + 1) }\n";
    let direct_order = "package p\nfunc f(x int) { record(x); record(x + 1) }\n";
    let select_default =
        "package p\nfunc f(ch <-chan int) int { select { case v := <-ch: return v; default: return 0 } }\n";
    let direct_receive = "package p\nfunc f(ch <-chan int) int { v := <-ch; return v }\n";
    let nil_channel = "package p\nfunc f() int { var ch chan int; return <-ch }\n";
    let zero_value = "package p\nfunc f() int { return 0 }\n";

    assert_ne!(
        value_fp(&i, receive_value, Lang::Go),
        value_fp(&i, receive_status, Lang::Go),
        "Go receive value and comma-ok status expose different channel obligations"
    );
    assert_ne!(
        value_fp(&i, send, Lang::Go),
        value_fp(&i, assign, Lang::Go),
        "Go channel send is synchronization, not ordinary assignment"
    );
    assert_ne!(
        value_fp(&i, goroutine, Lang::Go),
        value_fp(&i, direct_call, Lang::Go),
        "Go goroutine scheduling must not merge with direct calls"
    );
    assert_ne!(
        value_fp(&i, deferred, Lang::Go),
        value_fp(&i, direct_order, Lang::Go),
        "Go defer changes callback timing and panic/return interaction"
    );
    assert_ne!(
        value_fp(&i, select_default, Lang::Go),
        value_fp(&i, direct_receive, Lang::Go),
        "Go select readiness/default semantics differ from a direct receive"
    );
    assert_ne!(
        value_fp(&i, nil_channel, Lang::Go),
        value_fp(&i, zero_value, Lang::Go),
        "Go nil-channel blocking must not collapse into an ordinary zero value"
    );
}

#[test]
fn python_asyncio_and_async_protocol_lifecycles_stay_split() {
    let i = Interner::new();
    let gather = "import asyncio\nasync def f(a, b):\n    return await asyncio.gather(a, b)\n";
    let wait_first = "import asyncio\nasync def f(a, b):\n    done, pending = await asyncio.wait([a, b], return_when=asyncio.FIRST_COMPLETED)\n    return done\n";
    let sleep = "import asyncio\nasync def f():\n    return await asyncio.sleep(1)\n";
    let imported_sleep = "from asyncio import sleep\nasync def f():\n    return await sleep(1)\n";
    let direct_none = "async def f():\n    return None\n";
    let async_for = "async def f(xs):\n    async for x in xs:\n        return x\n    return None\n";
    let sync_for = "async def f(xs):\n    for x in xs:\n        return x\n    return None\n";
    let async_with = "async def f(cm):\n    async with cm:\n        return 1\n";
    let sync_with = "async def f(cm):\n    with cm:\n        return 1\n";
    let wait_for =
        "import asyncio\nasync def f(task):\n    return await asyncio.wait_for(task, 1)\n";
    let shield = "import asyncio\nasync def f(task):\n    return await asyncio.shield(task)\n";
    let create_task = "import asyncio\nasync def f(coro):\n    return asyncio.create_task(coro)\n";
    let ensure_future =
        "import asyncio\nasync def f(coro):\n    return asyncio.ensure_future(coro)\n";
    let run_coro_threadsafe =
        "import asyncio\nasync def f(coro, loop):\n    return asyncio.run_coroutine_threadsafe(coro, loop)\n";
    let to_thread = "import asyncio\nasync def f(fn):\n    return await asyncio.to_thread(fn)\n";
    let run_coro = "import asyncio\ndef f(coro):\n    return asyncio.run(coro)\n";
    let return_coro = "def f(coro):\n    return coro\n";
    let generator_yield = "def f(xs):\n    for x in xs:\n        yield x\n";
    let list_materialized = "def f(xs):\n    return [x for x in xs]\n";

    assert_ne!(
        value_fp(&i, gather, Lang::Python),
        value_fp(&i, wait_first, Lang::Python),
        "asyncio.gather all-result ordering must not merge with wait first-completed behavior"
    );
    assert_ne!(
        value_fp(&i, sleep, Lang::Python),
        value_fp(&i, direct_none, Lang::Python),
        "asyncio.sleep timer scheduling must not merge with direct None"
    );
    assert_ne!(
        value_fp(&i, imported_sleep, Lang::Python),
        value_fp(&i, direct_none, Lang::Python),
        "imported asyncio.sleep bindings keep the same timer boundary as asyncio.sleep"
    );
    assert_ne!(
        value_fp(&i, async_for, Lang::Python),
        value_fp(&i, sync_for, Lang::Python),
        "async iteration lifecycle must remain separate from ordinary iteration"
    );
    assert_ne!(
        value_fp(&i, async_with, Lang::Python),
        value_fp(&i, sync_with, Lang::Python),
        "async context-manager cleanup and exception channels differ from ordinary with"
    );
    assert_ne!(
        value_fp(&i, wait_for, Lang::Python),
        value_fp(&i, shield, Lang::Python),
        "asyncio.wait_for timeout and shield cancellation propagation differ"
    );
    assert_ne!(
        value_fp(&i, create_task, Lang::Python),
        value_fp(&i, ensure_future, Lang::Python),
        "asyncio.create_task and ensure_future have adjacent but distinct task lifecycle contracts"
    );
    assert_ne!(
        value_fp(&i, run_coro_threadsafe, Lang::Python),
        value_fp(&i, to_thread, Lang::Python),
        "thread-safe coroutine scheduling and thread offload must stay separate"
    );
    assert_ne!(
        value_fp(&i, run_coro, Lang::Python),
        value_fp(&i, return_coro, Lang::Python),
        "asyncio.run drives a coroutine while returning it leaves it undriven"
    );
    assert_ne!(
        value_fp(&i, generator_yield, Lang::Python),
        value_fp(&i, list_materialized, Lang::Python),
        "generator yield lifecycle must not collapse into eager materialization"
    );
}

#[test]
fn rust_future_drive_spawn_join_and_select_boundaries_stay_split() {
    let i = Interner::new();
    let spawn = "async fn f(x: i32) { tokio::spawn(async move { work(x).await; }); }\n";
    let direct = "async fn f(x: i32) { work(x).await; }\n";
    let join =
        "async fn f(a: Fut, b: Fut) -> (Result<i32, E>, Result<i32, E>) { tokio::join!(a, b) }\n";
    let try_join =
        "async fn f(a: Fut, b: Fut) -> Result<(i32, i32), E> { tokio::try_join!(a, b) }\n";
    let select = "async fn f(a: Fut, b: Fut) -> Result<i32, E> { tokio::select! { v = a => v, v = b => v } }\n";
    let block_on = "fn f(rt: tokio::runtime::Runtime, fut: F) { rt.block_on(fut); }\n";
    let return_future = "fn f(rt: tokio::runtime::Runtime, fut: F) -> F { fut }\n";
    let nested_runtime =
        "fn f(rt: tokio::runtime::Runtime, fut: F) { rt.block_on(async { rt.block_on(fut); }); }\n";
    let wrapper_returned_runtime =
        "fn f(fut: F) { make().block_on(fut); }\nfn make() -> tokio::runtime::Runtime { tokio::runtime::Runtime::new().unwrap() }\n";
    let local_runtime =
        "fn f(fut: F) { let rt = tokio::runtime::Runtime::new().unwrap(); rt.block_on(fut); }\n";
    let constructor_assigned_field =
        "fn f(fut: F) { let runner = Runner::new(tokio::runtime::Runtime::new().unwrap()); runner.rt.block_on(fut); }\nstruct Runner { rt: tokio::runtime::Runtime }\nimpl Runner { fn new(rt: tokio::runtime::Runtime) -> Self { Self { rt } } }\n";
    let direct_field_assignment =
        "fn f(fut: F) { let runner = Runner { rt: tokio::runtime::Runtime::new().unwrap() }; runner.rt.block_on(fut); }\nstruct Runner { rt: tokio::runtime::Runtime }\n";

    assert_ne!(
        value_fp(&i, spawn, Lang::Rust),
        value_fp(&i, direct, Lang::Rust),
        "Rust spawn handle lifecycle and detached effects must not merge with direct await"
    );
    assert_ne!(
        value_fp(&i, join, Lang::Rust),
        value_fp(&i, select, Lang::Rust),
        "Rust join all-settled behavior differs from select first-ready behavior"
    );
    assert_ne!(
        value_fp(&i, join, Lang::Rust),
        value_fp(&i, try_join, Lang::Rust),
        "Rust join and try_join keep ordinary and short-circuit error channels split"
    );
    assert_ne!(
        value_fp(&i, block_on, Lang::Rust),
        value_fp(&i, return_future, Lang::Rust),
        "block_on drives a Future; returning the Future leaves it undriven"
    );
    assert_ne!(
        value_fp(&i, nested_runtime, Lang::Rust),
        value_fp(&i, block_on, Lang::Rust),
        "nested runtime drive hazards need explicit scheduling proof before block_on exact recovery"
    );
    assert_ne!(
        value_fp_named(&i, wrapper_returned_runtime, Lang::Rust, "f"),
        value_fp_named(&i, local_runtime, Lang::Rust, "f"),
        "wrapper-returned Runtime values need callable result provenance before receiver proof can widen"
    );
    assert_ne!(
        value_fp_named(&i, constructor_assigned_field, Lang::Rust, "f"),
        value_fp_named(&i, direct_field_assignment, Lang::Rust, "f"),
        "constructor-assigned Runtime fields need field lifecycle proof before inheriting direct field proof"
    );
}

#[test]
fn java_future_executor_and_stream_lifecycles_stay_split() {
    let i = Interner::new();
    let future_callback = "import java.util.concurrent.*;\nclass C { static CompletableFuture<Integer> f(Integer x) { return CompletableFuture.completedFuture(x).thenApply(v -> v + 1); } }\n";
    let direct_function =
        "import java.util.function.*;\nclass C { static Integer f(Integer x) { Function<Integer, Integer> fn = v -> v + 1; return fn.apply(x); } }\n";
    let sync_callback =
        "import java.util.concurrent.*;\nclass C { static void f(Runnable r) { r.run(); } }\n";
    let executor_execute =
        "import java.util.concurrent.*;\nclass C { static void f(Executor e, Runnable r) { e.execute(r); } }\n";
    let future_get = "import java.util.concurrent.*;\nclass C { static Object f(Future<Object> f) throws Exception { return f.get(); } }\n";
    let future_cancel =
        "import java.util.concurrent.*;\nclass C { static boolean f(Future<Object> f) { return f.cancel(true); } }\n";
    let future_get_timeout = "import java.util.concurrent.*;\nclass C { static Object f(Future<Object> f) throws Exception { return f.get(1, TimeUnit.SECONDS); } }\n";
    let future_status =
        "import java.util.concurrent.*;\nclass C { static boolean f(Future<Object> f) { return f.isDone(); } }\n";
    let handle = "import java.util.concurrent.*;\nclass C { static CompletableFuture<Integer> f(CompletableFuture<Integer> p) { return p.handle((v, e) -> v + 1); } }\n";
    let when_complete = "import java.util.concurrent.*;\nclass C { static CompletableFuture<Integer> f(CompletableFuture<Integer> p) { return p.whenComplete((v, e) -> observe(e)); } }\n";
    let all_of = "import java.util.concurrent.*;\nclass C { static CompletableFuture<Void> f(CompletableFuture<?> a, CompletableFuture<?> b) { return CompletableFuture.allOf(a, b); } }\n";
    let any_of = "import java.util.concurrent.*;\nclass C { static CompletableFuture<Object> f(CompletableFuture<?> a, CompletableFuture<?> b) { return CompletableFuture.anyOf(a, b); } }\n";
    let schedule = "import java.util.concurrent.*;\nclass C { static ScheduledFuture<?> f(ScheduledExecutorService e, Runnable r) { return e.schedule(r, 1, TimeUnit.SECONDS); } }\n";
    let interval = "import java.util.concurrent.*;\nclass C { static ScheduledFuture<?> f(ScheduledExecutorService e, Runnable r) { return e.scheduleAtFixedRate(r, 0, 1, TimeUnit.SECONDS); } }\n";
    let stream = "import java.util.*;\nclass C { static Object f(List<Integer> xs) { return xs.stream().map(x -> x + 1); } }\n";
    let parallel =
        "import java.util.*;\nclass C { static Object f(List<Integer> xs) { return xs.parallelStream().map(x -> x + 1); } }\n";

    assert_ne!(
        value_fp(&i, executor_execute, Lang::Java),
        value_fp(&i, sync_callback, Lang::Java),
        "Executor callback scheduling must not merge with direct Runnable.run invocation"
    );
    assert_ne!(
        value_fp(&i, future_callback, Lang::Java),
        value_fp(&i, direct_function, Lang::Java),
        "CompletionStage continuations must not collapse into synchronous Function.apply callbacks"
    );
    assert_ne!(
        value_fp(&i, future_callback, Lang::Java),
        value_fp(&i, executor_execute, Lang::Java),
        "CompletionStage value continuations and Executor callback scheduling expose different future obligations"
    );
    assert_ne!(
        value_fp(&i, future_get, Lang::Java),
        value_fp(&i, future_cancel, Lang::Java),
        "Future.get and cancellation/status channels must remain separate"
    );
    assert_ne!(
        value_fp(&i, future_get_timeout, Lang::Java),
        value_fp(&i, future_status, Lang::Java),
        "Future timeout waits and status checks expose different lifecycle channels"
    );
    assert_ne!(
        value_fp(&i, handle, Lang::Java),
        value_fp(&i, when_complete, Lang::Java),
        "CompletionStage handle and whenComplete differ in value recovery and observation effects"
    );
    assert_ne!(
        value_fp(&i, all_of, Lang::Java),
        value_fp(&i, any_of, Lang::Java),
        "CompletableFuture allOf and anyOf must keep all-completion and first-completion channels split"
    );
    assert_ne!(
        value_fp(&i, schedule, Lang::Java),
        value_fp(&i, interval, Lang::Java),
        "ScheduledExecutor one-shot delay and interval lifecycle are distinct"
    );
    assert_ne!(
        value_fp(&i, stream, Lang::Java),
        value_fp(&i, parallel, Lang::Java),
        "sequential stream and parallelStream require separate lifecycle/effect proof"
    );
}

#[test]
fn swift_task_async_try_and_continuation_boundaries_stay_split() {
    let i = Interner::new();
    let task_spawn = "func f() async { Task { await work() } }\n";
    let detached_task = "func f() async { Task.detached { await work() } }\n";
    let direct_await = "func f() async { await work() }\n";
    let async_let = "func f() async { async let x = work(); _ = await x }\n";
    let local_await = "func f() async { let x = await work(); _ = x }\n";
    let async_sequence = "func f(xs: AsyncStream<Int>) async { for await x in xs { use(x) } }\n";
    let sync_sequence = "func f(xs: [Int]) { for x in xs { use(x) } }\n";
    let throwing_try = "func f() async throws -> Int { return try await load() }\n";
    let plain_await = "func f() async -> Int { return await load() }\n";
    let checked_continuation =
        "func f() async -> Int { return await withCheckedContinuation { c in c.resume(returning: 1) } }\n";
    let throwing_continuation =
        "func f() async throws -> Int { return try await withCheckedThrowingContinuation { c in c.resume(throwing: error) } }\n";

    assert_ne!(
        value_fp(&i, task_spawn, Lang::Swift),
        value_fp(&i, direct_await, Lang::Swift),
        "Swift Task scheduling is not a direct await"
    );
    assert_ne!(
        value_fp(&i, task_spawn, Lang::Swift),
        value_fp(&i, detached_task, Lang::Swift),
        "Swift Task and Task.detached have distinct scheduling/inheritance obligations"
    );
    assert_ne!(
        value_fp(&i, detached_task, Lang::Swift),
        value_fp(&i, direct_await, Lang::Swift),
        "Swift detached tasks also remain separate from direct await"
    );
    assert_ne!(
        value_fp(&i, async_let, Lang::Swift),
        value_fp(&i, local_await, Lang::Swift),
        "Swift async let creates child-task lifecycle obligations beyond local await"
    );
    assert_ne!(
        value_fp(&i, async_sequence, Lang::Swift),
        value_fp(&i, sync_sequence, Lang::Swift),
        "Swift async sequence iteration must not merge with ordinary sequence iteration"
    );
    assert_ne!(
        value_fp(&i, throwing_try, Lang::Swift),
        value_fp(&i, plain_await, Lang::Swift),
        "Swift try await keeps exception-channel and scheduling obligations visible"
    );
    assert_ne!(
        value_fp(&i, checked_continuation, Lang::Swift),
        value_fp(&i, throwing_continuation, Lang::Swift),
        "throwing and non-throwing continuations expose different settlement channels"
    );
}

#[test]
fn ruby_thread_fiber_yield_and_exception_boundaries_stay_split() {
    let i = Interner::new();
    let thread = "def f(x)\n  Thread.new { work(x) }\nend\n";
    let direct = "def f(x)\n  work(x)\nend\n";
    let fiber_yield = "def f(x)\n  Fiber.yield(x)\nend\n";
    let block_yield = "def f(x, &block)\n  yield x\nend\n";
    let raise_value = "def f(x)\n  raise x\nend\n";
    let return_value = "def f(x)\n  return x\nend\n";
    let rescue_value = "def f(x)\n  begin\n    work(x)\n  rescue\n    recover(x)\n  end\nend\n";
    let direct_rescue_body = "def f(x)\n  work(x)\n  recover(x)\nend\n";
    let ensure_cleanup = "def f(x)\n  begin\n    work(x)\n  ensure\n    cleanup(x)\n  end\nend\n";
    let direct_cleanup = "def f(x)\n  work(x)\n  cleanup(x)\nend\n";
    let callback_consumed = "def f(xs)\n  xs.map { |x| work(x) }\nend\n";
    let callback_ignored = "def f(xs)\n  xs.each { |x| work(x) }\nend\n";

    assert_ne!(
        value_fp(&i, thread, Lang::Ruby),
        value_fp(&i, direct, Lang::Ruby),
        "Ruby Thread scheduling must not merge with direct method calls"
    );
    assert_ne!(
        value_fp(&i, fiber_yield, Lang::Ruby),
        value_fp(&i, block_yield, Lang::Ruby),
        "Fiber yield/resume lifecycle differs from block callback demand"
    );
    assert_ne!(
        value_fp(&i, raise_value, Lang::Ruby),
        value_fp(&i, return_value, Lang::Ruby),
        "Ruby raise and return use different exception/success channels"
    );
    assert_ne!(
        value_fp(&i, rescue_value, Lang::Ruby),
        value_fp(&i, direct_rescue_body, Lang::Ruby),
        "Ruby rescue observes exception-channel ordering rather than ordinary sequencing"
    );
    assert_ne!(
        value_fp(&i, ensure_cleanup, Lang::Ruby),
        value_fp(&i, direct_cleanup, Lang::Ruby),
        "Ruby ensure cleanup ordering is not ordinary sequential cleanup"
    );
    assert_ne!(
        value_fp(&i, callback_consumed, Lang::Ruby),
        value_fp(&i, callback_ignored, Lang::Ruby),
        "Ruby callback results consumed by map must not merge with ignored each callbacks"
    );
}
