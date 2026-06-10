//! Generic async-job status — pure logic (no UI, no I/O).
//!
//! `JobStatus` plus the `upsert_job`/`demo_advance_jobs` reducers that drive the
//! background-job spine. The rail (`rail.rs`) renders one icon per job from the
//! resulting `&[JobStatus]`; this module owns only the state transitions. The
//! status surface is the *visual proof of the background-job spine* — strictly
//! generic async-job status, with **no** agent/LLM-specific UI, labels, or demo
//! (see `docs/overlay.md` CHILD #2). The `JobStatus` variants are FROZEN — the
//! demo spine in `mod.rs` constructs them, so they must not be renamed or removed.

use gpui::SharedString;

/// Status of a single generic background job, rendered as one icon in the rail.
/// Strictly generic — no agent/LLM-specific framing (see `docs/overlay.md` CHILD #2).
///
/// The variant set is FROZEN: the demo spine in `mod.rs` and the `upsert_job`
/// reducer below construct these, so do not rename or remove them.
#[derive(Clone, Debug, PartialEq)]
pub enum JobStatus {
    Idle,
    Running { note: SharedString },
    Done(SharedString),
    Failed(SharedString),
}

impl JobStatus {
    /// Whether this job is actively running. Drives the per-icon pulse animation.
    pub fn is_running(&self) -> bool {
        matches!(self, JobStatus::Running { .. })
    }
}

/// Insert-or-replace the job at `idx`, extending the vec with `JobStatus::Idle`
/// placeholders if `idx` is past the current end. Pure reducer — no UI, no I/O.
/// This is the single mutation point the background-job spine drives; rendering
/// then derives entirely from the resulting `&[JobStatus]`.
pub fn upsert_job(jobs: &mut Vec<JobStatus>, idx: usize, next: JobStatus) {
    if idx >= jobs.len() {
        jobs.resize(idx + 1, JobStatus::Idle);
    }
    jobs[idx] = next;
}

/// Demo driver for the background-job spine: starting from empty, seed ~3 jobs and
/// cycle each one Idle -> Running -> Done on successive calls. Integration wires this
/// into `mod.rs`'s `TopRight` branch so the rail visibly animates without any
/// agent/LLM framing — it is purely the generic async-job proof.
///
/// The advance is order-preserving and idempotent at the terminal state: the first
/// not-yet-`Done` job steps forward one stage; once all are `Done` it is a no-op.
pub fn demo_advance_jobs(jobs: &mut Vec<JobStatus>) {
    const DEMO_JOB_COUNT: usize = 3;

    // First call (or any time the vec is short): seed the placeholders.
    if jobs.len() < DEMO_JOB_COUNT {
        jobs.resize(DEMO_JOB_COUNT, JobStatus::Idle);
        return;
    }

    // Advance the first job that has not finished yet, one stage at a time. This
    // staggers the jobs so the rail shows a mix of Idle/Running/Done as it fills in.
    for idx in 0..jobs.len() {
        let next = match &jobs[idx] {
            JobStatus::Idle => Some(JobStatus::Running {
                note: SharedString::from("working"),
            }),
            // The last seeded job fails, so the rail demonstrates the failure state
            // (danger color) too; the earlier jobs complete successfully.
            JobStatus::Running { .. } if idx + 1 == jobs.len() => {
                Some(JobStatus::Failed(SharedString::from("error")))
            }
            JobStatus::Running { .. } => Some(JobStatus::Done(SharedString::from("ok"))),
            // Already settled — look at the next job.
            JobStatus::Done(_) | JobStatus::Failed(_) => None,
        };
        if let Some(next) = next {
            upsert_job(jobs, idx, next);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_job_inserts_replaces_and_extends() {
        let mut jobs: Vec<JobStatus> = Vec::new();

        // Insert at the next slot.
        upsert_job(&mut jobs, 0, JobStatus::Running { note: "a".into() });
        assert_eq!(jobs, vec![JobStatus::Running { note: "a".into() }]);

        // Replace in place.
        upsert_job(&mut jobs, 0, JobStatus::Done("a".into()));
        assert_eq!(jobs, vec![JobStatus::Done("a".into())]);

        // Extend past the end — the gap fills with `Idle` placeholders.
        upsert_job(&mut jobs, 2, JobStatus::Failed("c".into()));
        assert_eq!(
            jobs,
            vec![
                JobStatus::Done("a".into()),
                JobStatus::Idle,
                JobStatus::Failed("c".into()),
            ]
        );
    }

    #[test]
    fn demo_advance_seeds_then_cycles_a_job_idle_running_done() {
        let mut jobs: Vec<JobStatus> = Vec::new();

        // First call seeds the placeholders; nothing is running yet.
        demo_advance_jobs(&mut jobs);
        assert_eq!(jobs.len(), 3);
        assert!(jobs.iter().all(|j| *j == JobStatus::Idle));

        // Idle -> Running for the first job.
        demo_advance_jobs(&mut jobs);
        assert!(jobs[0].is_running());
        assert_eq!(jobs[1], JobStatus::Idle);

        // Running -> Done for the first job (the rest stay untouched this step).
        demo_advance_jobs(&mut jobs);
        assert_eq!(jobs[0], JobStatus::Done("ok".into()));

        // Driving it to completion eventually settles every job (no panic, terminal
        // state is a no-op). The last seeded job settles to `Failed`, the rest `Done`.
        for _ in 0..16 {
            demo_advance_jobs(&mut jobs);
        }
        assert!(jobs
            .iter()
            .all(|j| matches!(j, JobStatus::Done(_) | JobStatus::Failed(_)) && !j.is_running()));
        assert!(matches!(jobs.last(), Some(JobStatus::Failed(_))));
    }

    #[test]
    fn is_running_is_true_only_for_running() {
        assert!(JobStatus::Running { note: "x".into() }.is_running());
        assert!(!JobStatus::Idle.is_running());
        assert!(!JobStatus::Done("x".into()).is_running());
        assert!(!JobStatus::Failed("x".into()).is_running());
    }
}
