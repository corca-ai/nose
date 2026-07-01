use super::{missing_evidence_for_lang_call, runtime_boundary_evidence_for_lang_call};
use nose_il::Lang;

#[test]
fn ruby_thread_and_fiber_calls_report_shared_task_obligations() {
    for (src, callee) in [
        ("def run\n  Thread.new { work }\nend\n", "Thread.new"),
        ("def run\n  Thread.start { work }\nend\n", "Thread.start"),
        ("def run\n  Thread.fork { work }\nend\n", "Thread.fork"),
        ("def run\n  Fiber.new { work }\nend\n", "Fiber.new"),
        (
            "def run\n  Fiber.schedule { work }\nend\n",
            "Fiber.schedule",
        ),
    ] {
        let labels = missing_evidence_for_lang_call("runtime.rb", src, Lang::Ruby, callee);
        assert!(labels.contains(&"task-spawn-scheduling-contract"));
        assert!(labels.contains(&"task-handle-lifecycle-contract"));
        assert!(labels.contains(&"task-cancellation-liveness-contract"));
        assert!(labels.contains(&"concurrency-scheduling-contract"));
    }
}

#[test]
fn ruby_thread_and_fiber_calls_require_unshadowed_runtime_roots() {
    for (src, callee) in [
        (
            "Thread = Struct.new(:value)\ndef run\n  Thread.new { work }\nend\n",
            "Thread.new",
        ),
        (
            "class Thread\nend\ndef run\n  Thread.start { work }\nend\n",
            "Thread.start",
        ),
        (
            "module M\n  def self.run\n    Thread.new { work }\n  end\nend\nclass M::Thread\nend\n",
            "Thread.new",
        ),
        (
            "module M\n  const_set(:Thread, Struct.new(:value))\n  def self.run\n    Thread.new { work }\n  end\nend\n",
            "Thread.new",
        ),
        (
            "Fiber = Struct.new(:value)\ndef run\n  Fiber.new { work }\nend\n",
            "Fiber.new",
        ),
        (
            "class Fiber\nend\ndef run\n  Fiber.schedule { work }\nend\n",
            "Fiber.schedule",
        ),
        (
            "module M\n  def self.run\n    Fiber.schedule { work }\n  end\nend\nclass M::Fiber\nend\n",
            "Fiber.schedule",
        ),
        (
            "module M\n  M.const_set(\"Fiber\", Struct.new(:value))\n  def self.run\n    Fiber.schedule { work }\n  end\nend\n",
            "Fiber.schedule",
        ),
    ] {
        let labels = runtime_boundary_evidence_for_lang_call("runtime.rb", src, Lang::Ruby, callee);
        assert!(
            labels.is_none(),
            "{callee} with a same-file Ruby runtime root definition should remain closed: {labels:?}"
        );
    }
}

#[test]
fn ruby_thread_observer_calls_do_not_report_spawn_obligations() {
    let labels = runtime_boundary_evidence_for_lang_call(
        "runtime.rb",
        "def run\n  Thread.current[:request_id]\nend\n",
        Lang::Ruby,
        "Thread.current",
    );
    assert!(
        labels.is_none(),
        "Thread.current should not be treated as a spawn boundary: {labels:?}"
    );
}
