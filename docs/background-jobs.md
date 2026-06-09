# Background jobs in Deck

How to run work that outlives a keystroke — a multi-turn agent talking to an API for
minutes, a listener that waits hours for a signal — without freezing the UI or pulling in
a second async runtime.

**The omakase take:** GPUI already *is* your async runtime. Deck ships an opinionated,
zero-dependency job pattern built on it, and documents two escape hatches for when you want
the tokio ecosystem or a different stack. Pick the default, or swap a layer — nothing here
locks you in.

> Performance rule first (see `docs/LEARNINGS.md` §17 and `CLAUDE.md`): **never block the
> render thread on I/O.** Everything below runs work off the UI thread and pushes results
> back with `cx.notify()`. That rule is why this doc exists.

---

## 1. You already have a runtime — don't add one

GPUI bundles a full executor (the same one Zed runs its editor, language servers, and agent
panel on). You do **not** need `tokio`, `async-std`, or a thread-pool crate for the runtime
itself. Verified API (`gpui/src/executor.rs`, `gpui/src/app/async_context.rs`):

| You want | Call | Notes |
|---|---|---|
| Run hours-long / blocking / CPU work off the UI thread | `cx.background_executor().spawn(fut)` | future must be `Send + 'static` (`executor.rs:89`) |
| Run async work that touches UI state | `cx.spawn(async move \|cx\| { … })` → `AsyncApp` | main-thread, future need not be `Send` (`async_context.rs:204`) |
| Sleep / poll | `cx.background_executor().timer(Duration)` | returns `Task<()>` (`executor.rs:162`); **never** `std::thread::sleep` on the UI thread |
| Bounded wait | `executor.block_with_timeout(dur, fut)` | `executor.rs:364` |
| Fire-and-forget with error logging | `task.detach_and_log_err(cx)` | `TaskExt`, `executor.rs:33` |

**`Task<T>` is the handle, and dropping it cancels the work** (at the next `.await` point).
That gives you structured cancellation for free: hold the `Task` on an entity, drop the
entity (or replace the `Task`), and the job stops. Call `.detach()` only when you truly want
it to outlive its handle. `tray.rs:62-78` is the canonical "listen forever, push to UI" loop.

---

## 2. The omakase pattern: a cancellable job that reports status

The shape Deck recommends for any long-running job (an agent turn loop, a watcher, an
import): an entity owns a `Task` plus a status enum; the job runs on the **background**
executor and pushes status back through a **`WeakEntity` + `cx.notify()`** (the same spine
the overlay's agent-row uses — see `docs/overlay.md` Child #1).

```rust
pub enum JobStatus {
    Idle,
    Running { turn: usize, note: SharedString },
    Retrying { attempt: u32, after: Duration },
    Failed(SharedString),
    Done(SharedString),
}

pub struct AgentJob {
    status: JobStatus,
    task: Option<Task<()>>, // dropping this cancels the run
}

impl AgentJob {
    pub fn start(&mut self, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();                 // WeakEntity<AgentJob>
        self.task = Some(cx.background_executor().spawn(async move {
            let mut attempt = 0;
            loop {
                match run_one_turn().await {                // your async API call
                    Ok(turn) => {
                        // hop back to the app to mutate UI state + repaint
                        let _ = this.update(/* AsyncApp */ cx, |job, cx| {
                            job.status = JobStatus::Running { turn, note: "ok".into() };
                            cx.notify();                     // smallest entity that changed
                        });
                        if turn.is_final() { break; }
                        attempt = 0;
                    }
                    Err(e) if attempt < MAX_RETRIES => {
                        attempt += 1;
                        let backoff = retry_backoff(attempt); // pure, unit-tested below
                        // … set Retrying status via this.update(…) …
                        cx.background_executor().timer(backoff).await;
                    }
                    Err(e) => { /* set Failed via this.update(…); */ break; }
                }
            }
        }));
        // Note: the closure above needs an AsyncApp to call `this.update`; in real code use
        // `cx.spawn(async move |cx| { … cx.background_executor().spawn(...) … })` so you hold
        // an AsyncApp, or pass results out via a channel and apply them on the foreground.
    }

    pub fn cancel(&mut self) { self.task = None; } // drop = cancel
}
```

Two true things to internalize:
- **`WeakEntity::upgrade()` returning `None` is your "UI is gone" signal** — pushes after the
  window/entity closes simply no-op. No leaks, no panics. (This is exactly how the overlay
  spec handles a closed HUD.)
- **`this.update(...)` returns a `Result`** — handle it (`?` or `let _ =`); a dropped
  fallible result trips `unused_must_use` (denied in this repo).

### Retry/backoff is pure and testable (zero-dep)

Keep the backoff math out of the async closure so it unit-tests like
`command_palette::fuzzy()`:

```rust
fn retry_backoff(attempt: u32) -> Duration {
    // exp backoff, capped — swap for the `backoff` crate if you want jitter/decorrelation
    let secs = (1u64 << attempt.min(6)).min(60);
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    #[test] fn backoff_grows_then_caps() { /* 2,4,8,…,60,60 */ }
    #[test] fn status_transitions_terminal() { /* Failed/Done don't resume */ }
}
```

---

## 3. The HTTP / API client — the one real decision

GPUI runs *futures* but ships **no HTTP client**. Deck is UI-only today, so this is
greenfield. Three stacks, each a deliberate tradeoff:

| Option | How | Pros | Cons | Dep cost |
|---|---|---|---|---|
| **`gpui_tokio` + reqwest / official SDKs** *(recommended for agents)* | `gpui_tokio::init(cx)` once, then `Tokio::spawn(cx, async { reqwest … }) -> Task<Result<R, JoinError>>` (`gpui_tokio.rs:55`) | Unlocks the mature tokio ecosystem: `reqwest`, `async-openai`, the Anthropic SDKs, streaming, TLS. Results come back as GPUI `Task`s. | Runs a second (tokio) runtime alongside GPUI's. New deps. | new: `gpui_tokio` (first-party) + `reqwest`/SDK — approval-gated (DoD #4) |
| **Blocking client on the bg pool** | `cx.background_executor().spawn(\|\| ureq::post(...))` | Dead simple, robust, no second runtime. The bg pool tolerates blocking. | A blocking call **can't cancel mid-flight** (drop only cancels at await points). No streaming ergonomics. | new: `ureq` — approval-gated |
| **Zed's `http_client`** | the `http_client` crate in the gpui stack | Already in the wider tree; matches Zed's abstraction. | Heavier, Zed-shaped API; more than a starter needs. | new: `http_client` |

**Omakase pick:** `gpui_tokio` + `reqwest` (or the official Anthropic SDK) for anything
agent-shaped — multi-turn, streaming, cancellation all work cleanly. It's a first-party gpui
crate, so the bridge is low-risk; the SDK is your call. Reach for blocking `ureq` only for a
one-shot call where simplicity beats cancellability.

**Escape hatch (the "pick your own stack" promise):** the job pattern in §2 is
client-agnostic — it only cares that `run_one_turn()` returns a `Future`. Swap reqwest for
ureq, or tokio for a smol-native client, by changing that one function. Nothing else moves.

---

## 4. Cancellation, the honest version

- **Async path:** hold the `Task`; drop it to cancel at the next `.await`. For mid-request
  cancel, `futures::select!` the request against a cancel channel (`async-channel` /
  `flume`), or use `tokio_util::sync::CancellationToken` when on the tokio bridge.
- **Blocking path:** a `ureq`/blocking call on the bg pool **cannot** be interrupted; it runs
  to completion (or its own timeout). Set a request timeout; don't promise instant cancel.
- A **panic** inside a `Task` aborts that task only — the app survives — but there's **no
  auto-restart**. Wrap the loop body, convert errors to `Failed`, and let the user retry.

---

## 5. Listening for hours, and the macOS gotcha

A watcher (a socket, a channel, `notify` for filesystem events, a unix signal via
`signal-hook`) is just a background task that awaits in a loop and `cx.update`s on each event
— identical to `tray.rs`'s menu-event drain. Hours-long is fine: GPUI's bg thread pool is
persistent, and a native desktop process keeps its threads alive (no mobile/web sandbox
killing you).

**The one real platform caveat: macOS App Nap.** When your app is in the background, macOS
can throttle its timers and CPU (App Nap), which stalls a long agent run or a polling
listener. If you need guaranteed background progress, disable it via
`NSProcessInfo.processInfo.beginActivityWithOptions(.userInitiated | .idleSystemSleepDisabled, reason:)`
through objc2 — the same objc2 path `tray.rs:94-104` already uses for the dock policy. Hold
the returned activity token for the duration of the work, drop it when idle. Linux has no
equivalent throttle.

---

## 6. What Deck ships vs. what you add

- **Free, in the box (zero-dep):** the executor, `Task` cancellation, `timer`, the §2 job
  pattern, pure retry/backoff logic. The runtime is *not* a dependency decision.
- **Approval-gated (DoD #4 — new deps):** any HTTP client, `gpui_tokio`, official agent SDKs,
  `backoff`, `notify`, `signal-hook`, App-Nap objc2 (reuses the `tray` feature's objc2). Add
  them behind a feature flag (mirror `--features tray`) so the default fork stays lean.

The omakase default is: lean core + the §2 pattern, and you opt into a client stack when you
wire a real agent. That keeps "fork it, rename it, ship it" honest while leaving every stack
choice reversible.
