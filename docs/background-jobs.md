# Background jobs

How to run work that outlives a keystroke (an agent talking to an API for minutes, a
listener waiting hours for a signal) without freezing the UI or adding a second runtime.

Rule zero: **never block the UI thread on I/O.** Do the work off the UI thread and push
results back with `cx.notify()`.

## 1. You already have a runtime

GPUI ships its executor; you don't need tokio for the runtime itself.

| You want | Call | Notes |
|---|---|---|
| Async work that updates UI state | `cx.spawn(async move \|this, cx\| …)` | runs on the main thread; the future needn't be `Send` |
| Blocking or CPU-heavy work | `cx.background_executor().spawn(async move { … })` | thread pool; the future must be `Send + 'static` |
| Sleep | `cx.background_executor().timer(duration).await` | never `std::thread::sleep` on the UI thread |

**A `Task<T>` is a handle, and dropping it cancels the work** at the next `.await`. Keep the
task on the entity that owns the job: replacing or dropping it stops the job. `.detach()` only
what must outlive its owner (like the tray's event loop in `src/tray.rs`).

## 2. The pattern: a cancellable job that reports status

Illustrative (not compiled here): `run_one_turn`, `Turn` and `MAX_RETRIES` are yours.

```rust
pub enum JobStatus {
    Idle,
    Running { turn: usize },
    Retrying { attempt: u32 },
    Failed(SharedString),
    Done,
}

pub struct AgentJob {
    status: JobStatus,
    task: Option<Task<()>>, // dropping this cancels the run
}

impl AgentJob {
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.task = Some(cx.spawn(async move |this, cx| {
            let (mut turn, mut attempt) = (0, 0);
            loop {
                // The slow part runs on the thread pool; only the result comes back here.
                let result = cx.background_executor().spawn(run_one_turn(turn)).await;
                let status = match result {
                    Ok(Turn::Final) => JobStatus::Done,
                    Ok(Turn::More) => { turn += 1; attempt = 0; JobStatus::Running { turn } }
                    // Retry the same turn after a transient error.
                    Err(_) if attempt < MAX_RETRIES => { attempt += 1; JobStatus::Retrying { attempt } }
                    Err(err) => JobStatus::Failed(err.to_string().into()),
                };
                let finished = matches!(status, JobStatus::Done | JobStatus::Failed(_));
                // `Err` means the entity is gone (window closed): stop quietly.
                if this.update(cx, |job, cx| { job.status = status; cx.notify(); }).is_err() || finished {
                    return;
                }
                if attempt > 0 {
                    cx.background_executor().timer(retry_backoff(attempt)).await;
                }
            }
        }));
    }

    pub fn cancel(&mut self) {
        self.task = None; // drop = cancel
    }
}
```

- `this` is a `WeakEntity`: once the view is gone, `update` returns `Err` and the job stops.
- `update` returns a `Result`; handle it (`unused_must_use` is denied here).
- Keep logic like backoff pure, so it unit-tests without GPUI:

```rust
fn retry_backoff(attempt: u32) -> Duration {
    Duration::from_secs((1u64 << attempt.min(6)).min(60)) // 2, 4, 8 … capped at 60 s
}
```

## 3. The HTTP client: the one real decision

GPUI runs futures but gives you no HTTP client to depend on. Options, each a new dependency
(approval-gated):

| Option | How | Trade-off |
|---|---|---|
| Blocking client (`ureq`) on the thread pool | `background_executor().spawn(async move { ureq::post(…).send_json(…) })` | simplest, no second runtime; a request in flight can't be cancelled (set a timeout) |
| tokio + `reqwest` or an official SDK | own a `tokio::runtime::Runtime` (e.g. in a GPUI global), run requests on it, and await the result from `cx.spawn` through a channel or `JoinHandle` | streaming, TLS and the SDK ecosystem; a second runtime alongside GPUI's |

The job pattern above only cares that `run_one_turn()` is a future, so switching clients is a
one-function change.

## 4. Cancellation, honestly

- Async: drop the `Task`; it stops at the next `.await`. For mid-request cancel, race the
  request against a cancel channel.
- Blocking calls on the thread pool can't be interrupted; they finish or time out.
- Don't rely on panics being contained: a panic inside a GPUI task can take down the whole
  app (on macOS, GPUI runs tasks from an `extern "C"` callback, where a panic aborts). Return
  errors, turn them into `Failed`, and let the user retry.

## 5. Listening for hours, and macOS App Nap

A watcher (socket, channel, filesystem events) is a task that awaits in a loop and updates an
entity on each event, like `src/tray.rs`'s menu-event loop. GPUI's thread pool is persistent,
so hours are fine.

The macOS caveat: **App Nap** throttles timers and CPU for background apps, which stalls long
runs. Hold GPUI's guard while the work runs:

```rust
let _awake = cx.background_executor().prevent_app_nap("Agent run in progress");
```

It stops App Nap throttling, not system sleep. Linux has no equivalent throttle.
