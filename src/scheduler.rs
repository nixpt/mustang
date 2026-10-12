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
//! # Per-region rects
//!
//! Mutators that know *where* the DOM changed call
//! `on_region_changed(region)` instead of (or in addition to) the
//! region-less `on_dom_changed()`. Region-less mutations mark the
//! whole scene dirty; region-tracked mutations accumulate their rects.
//! The compositor consumes both via `take_dirty()`:
//!
//! - `SceneDirtyState::full == true` → re-check every region
//!   (region-less mutation supersedes any accumulated rects — they are
//!   drained, not returned).
//! - otherwise → the returned `regions` rects are the dirty set; the
//!   compositor re-applies only effects intersecting them.
//!
//! Mutations arriving between the dirty-check and the drain stay
//! pending for the next frame; nothing is lost, only deferred.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::compositor::region::Region;

/// The dirty state consumed by one application cycle.
///
/// Produced by [`SceneScheduler::take_dirty`]. A whole-scene mutation
/// (`full`) supersedes any accumulated region rects — the compositor
/// re-checks everything. Otherwise only `regions` are dirty; empty
/// `regions` + `full == false` means nothing to do.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneDirtyState {
    /// Whole-scene dirty flag — set by region-less `on_dom_changed()`.
    pub full: bool,
    /// Region rects mutated since the last application. Empty when
    /// `full` is `true` (whole-scene supersedes them) or when nothing
    /// is dirty.
    pub regions: Vec<Region>,
}

impl SceneDirtyState {
    /// `true` if the scene must be re-checked (wholly or per-region).
    pub fn is_dirty(&self) -> bool {
        self.full || !self.regions.is_empty()
    }
}

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
    /// Bumped on every `on_dom_changed()` / `on_region_changed()` call.
    dirty_count: AtomicU64,
    /// Set to `dirty_count` after the last successful effect application.
    last_applied: AtomicU64,
    /// Total number of times effects were actually re-applied.
    apply_count: AtomicU64,
    /// Total number of times the renderer asked `should_apply()`.
    query_count: AtomicU64,
    /// Whole-scene dirtiness flag — set by region-less `on_dom_changed()`,
    /// cleared on the next application. `AtomicBool::default()` is `false`,
    /// which is why `SchedulerInner` stays `Default`-derivable.
    full_dirty: AtomicBool,
    /// Region rects pending since the last application (from
    /// `on_region_changed`). Drained on the next application.
    pending_regions: Mutex<Vec<Region>>,
}

impl SceneScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal that the DOM has changed somewhere (region unknown) and
    /// effects may need to be re-applied. Marks the WHOLE scene dirty.
    /// Called by `DirectDomMutator` after a region-less mutation, or by
    /// the reactive `Reactor` after a flush that produced dirty patches
    /// without region metadata.
    ///
    /// For tracked mutations, prefer [`SceneScheduler::on_region_changed`]
    /// — whole-scene dirtiness supersedes region tracking.
    pub fn on_dom_changed(&self) {
        self.inner.full_dirty.store(true, Ordering::Release);
        self.inner.dirty_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Signal that a specific region of the scene changed. Accumulates
    /// the rect so the compositor can re-apply only the effects that
    /// intersect it, instead of re-checking the whole scene.
    pub fn on_region_changed(&self, region: Region) {
        self.inner
            .pending_regions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(region);
        self.inner.dirty_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Consume the pending dirty state and return it.
    ///
    /// Advances the scheduler exactly like [`SceneScheduler::should_apply`]
    /// — the next call returns a not-dirty state until another mutation
    /// happens — but also yields the per-region rects so the compositor
    /// can scope the re-application.
    pub fn take_dirty(&self) -> SceneDirtyState {
        self.inner.query_count.fetch_add(1, Ordering::Relaxed);
        self.consume()
    }

    fn consume(&self) -> SceneDirtyState {
        let dirty = self.inner.dirty_count.load(Ordering::Acquire);
        let applied = self.inner.last_applied.load(Ordering::Acquire);
        if dirty > applied {
            self.inner.last_applied.store(dirty, Ordering::Release);
            self.inner.apply_count.fetch_add(1, Ordering::Relaxed);

            // Drain pending regions first, then read the full-scene flag —
            // both must be consumed together. A whole-scene mutation
            // supersedes the rects (drain them, don't return them).
            let regions: Vec<Region> = self
                .inner
                .pending_regions
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .drain(..)
                .collect();
            let full = self.inner.full_dirty.swap(false, Ordering::AcqRel);
            if full {
                SceneDirtyState {
                    full: true,
                    regions: Vec::new(),
                }
            } else {
                SceneDirtyState {
                    full: false,
                    regions,
                }
            }
        } else {
            SceneDirtyState {
                full: false,
                regions: Vec::new(),
            }
        }
    }

    /// Called by the renderer's `set_scene_effects` hook before
    /// re-applying effects. Returns `true` if the DOM has changed since
    /// the last application (i.e. effects should be re-applied), or
    /// `false` if the cached state is still valid.
    ///
    /// When it returns `true`, the scheduler's dirty state is consumed
    /// (same as [`SceneScheduler::take_dirty`]) so the next call returns
    /// `false` until another mutation happens. Use [`SceneScheduler::take_dirty`]
    /// instead when the compositor needs the per-region rects, not just
    /// the boolean.
    pub fn should_apply(&self) -> bool {
        self.inner.query_count.fetch_add(1, Ordering::Relaxed);
        self.consume().is_dirty()
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

    #[test]
    fn region_change_yields_dirty_rects() {
        let s = SceneScheduler::new();
        s.on_region_changed(Region::new(0.0, 0.0, 50.0, 50.0));
        let state = s.take_dirty();
        assert!(state.is_dirty());
        assert!(!state.full, "region-tracked mutation is NOT whole-scene dirty");
        assert_eq!(state.regions.len(), 1);
        assert_eq!(state.regions[0], Region::new(0.0, 0.0, 50.0, 50.0));

        // Consumed: the next cycle is clean.
        assert!(!s.take_dirty().is_dirty());
    }

    #[test]
    fn region_mutations_coalesce_into_rect_list() {
        let s = SceneScheduler::new();
        s.on_region_changed(Region::new(0.0, 0.0, 50.0, 50.0));
        s.on_region_changed(Region::new(100.0, 0.0, 50.0, 50.0));
        s.on_region_changed(Region::new(0.0, 100.0, 50.0, 50.0));

        let state = s.take_dirty();
        assert_eq!(state.regions.len(), 3, "3 region mutations → 3 rects, one apply");
        assert_eq!(s.apply_count(), 1);
    }

    #[test]
    fn full_dirty_supersedes_regions() {
        let s = SceneScheduler::new();
        s.on_region_changed(Region::new(0.0, 0.0, 50.0, 50.0));
        // Region-less mutation after tracked ones: whole scene wins.
        s.on_dom_changed();

        let state = s.take_dirty();
        assert!(state.full, "region-less mutation marks whole scene dirty");
        assert!(state.regions.is_empty(), "rects are drained, not returned");

        // Both channels consumed — nothing pending after.
        assert!(!s.take_dirty().is_dirty());
    }

    #[test]
    fn regions_drain_without_full_dirty() {
        let s = SceneScheduler::new();
        s.on_region_changed(Region::new(10.0, 10.0, 20.0, 20.0));
        s.on_dom_changed();
        let state = s.take_dirty();
        assert!(state.full);
        // The earlier region was drained (not returned) with the full flag.
        assert!(state.regions.is_empty());
        // No residue: another region-only mutation starts a fresh cycle.
        s.on_region_changed(Region::new(0.0, 0.0, 5.0, 5.0));
        let state = s.take_dirty();
        assert!(!state.full);
        assert_eq!(state.regions, vec![Region::new(0.0, 0.0, 5.0, 5.0)]);
    }

    #[test]
    fn should_apply_and_take_dirty_share_the_consumption() {
        let s = SceneScheduler::new();
        s.on_region_changed(Region::new(0.0, 0.0, 10.0, 10.0));
        assert!(s.should_apply(), "boolean path sees region dirt");
        // The same mutation cannot apply twice through either path.
        assert!(!s.take_dirty().is_dirty());
        assert!(!s.should_apply());
    }
}
