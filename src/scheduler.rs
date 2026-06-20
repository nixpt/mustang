//! Unified Scene Scheduling — coordinates Mustang GPU effects with
//! DOM dirty-region tracking.
//!
//! The scheduler sits between the reactive layer and the renderer's
//! `set_scene_effects` hook. It tracks when the DOM has changed (via
//! the `DirectDomMutator` extension trait) and decides whether the
//! per-frame effect application should actually re-run, or whether
//! the cached effects from the previous frame are still valid.
//!
//! # Wiring
//!
//! 1. Create a `SceneScheduler` and hand it to the renderer via
//!    `VelloWindowRenderer::set_scene_effects` (the closure captures it).
//! 2. Hand the same `SceneScheduler` to the reactive `Reactor` (or to
//!    the `DirectDomMutator` extension trait) so that DOM mutations
//!    bump the scheduler's dirty counter.
//! 3. On each frame, the renderer's effect hook calls
//!    `scheduler.should_apply()`. If `true`, the compositor
//!    re-applies effects; if `false`, the cached `VelloScene`
//!    state is reused.
//!
//! # Phase 5
//!
//! This is the first cut. It tracks a single monotonic dirty counter
//! rather than per-region rects — per-region rects are a follow-up
//! once the wiring is proven out end-to-end.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Monotonic counter of DOM dirty events since the last effect application.
///
/// `SceneScheduler` is `Clone` (it's an `Arc` internally) so it can be
/// shared between the renderer effect hook, the reactive `Reactor`,
/// and the `DirectDomMutator` extension trait without lifetime gymnastics.
#[derive(Clone, Default)]
pub struct SceneScheduler {
    inner: Arc<SchedulerInner>,
}

#[derive(Default)]
struct SchedulerInner {
    /// Bumped on every `on_dom_changed()` call.
    dirty_count: AtomicU64,
    /// Set to `dirty_count` after the last successful effect application.
    last_applied: AtomicU64,
    /// Total number of times effects were actually re-applied.
    apply_count: AtomicU64,
    /// Total number of times the renderer asked `should_apply()`.
    query_count: AtomicU64,
}

impl SceneScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal that the DOM has changed and effects may need to be re-applied.
    /// Called by `DirectDomMutator` after a mutation, or by the reactive
    /// `Reactor` after a flush that produced dirty patches.
    pub fn on_dom_changed(&self) {
        self.inner.dirty_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Called by the renderer's `set_scene_effects` hook before
    /// re-applying effects. Returns `true` if the DOM has changed since
    /// the last application (i.e. effects should be re-applied), or
    /// `false` if the cached state is still valid.
    ///
    /// When it returns `true`, the scheduler's `last_applied` counter is
    /// advanced so the next call returns `false` until another mutation
    /// happens.
    pub fn should_apply(&self) -> bool {
        self.inner.query_count.fetch_add(1, Ordering::Relaxed);
        let dirty = self.inner.dirty_count.load(Ordering::Acquire);
        let applied = self.inner.last_applied.load(Ordering::Acquire);
        if dirty > applied {
            self.inner.last_applied.store(dirty, Ordering::Release);
            self.inner.apply_count.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Total number of times effects were actually re-applied.
    pub fn apply_count(&self) -> u64 {
        self.inner.apply_count.load(Ordering::Acquire)
    }

    /// Total number of times the renderer queried `should_apply()`.
    pub fn query_count(&self) -> u64 {
        self.inner.query_count.load(Ordering::Acquire)
    }

    /// Current dirty count (for diagnostics).
    pub fn dirty_count(&self) -> u64 {
        self.inner.dirty_count.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_scheduler_does_not_apply() {
        let s = SceneScheduler::new();
        assert!(!s.should_apply(), "no DOM change yet, no apply");
        assert_eq!(s.apply_count(), 0);
        assert_eq!(s.query_count(), 1);
    }

    #[test]
    fn dom_change_triggers_single_apply() {
        let s = SceneScheduler::new();
        s.on_dom_changed();
        assert!(s.should_apply(), "dirty > applied, should apply");
        assert!(!s.should_apply(), "advanced, no longer dirty");
        assert_eq!(s.apply_count(), 1);
        assert_eq!(s.query_count(), 2);
    }

    #[test]
    fn multiple_changes_coalesce() {
        let s = SceneScheduler::new();
        for _ in 0..10 {
            s.on_dom_changed();
        }
        assert!(s.should_apply());
        assert_eq!(s.apply_count(), 1, "10 mutations → 1 apply");
    }

    #[test]
    fn is_clone_and_send_sync() {
        let s = SceneScheduler::new();
        let s2 = s.clone();
        s.on_dom_changed();
        assert!(s2.should_apply());
    }
}
