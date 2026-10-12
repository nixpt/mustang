# Plan

Captured by agents as they work. An unchecked box is an open item;
nobody has committed to doing it, only to not losing it.

- [ ] **issue** — CONTRIBUTING mirrors: arniko (README boundary reciprocity, vendored crates stay) + bliss-engine (crates/bliss) need reciprocal what-THIS-repo-IS sections — cross-repo docs task, own session  _(unknown, 2026-10-11)_
- [ ] **issue** — published-crate CI: periodic drift check of crates.io arniko-mustang tarball vs repo source — gated on actual crates.io publication (needs CARGO_API_TOKEN secret + captain coordination)  _(unknown, 2026-10-11)_
- [ ] **issue** — PUBLISHED-CRATE DRIFT CI is now UNBLOCKED: arniko-mustang 0.3.0 is LIVE on crates.io (published 2026-09-27T07:43Z by publish.yml; verified via registry API, 36 downloads). Follow-up: periodic CI job that publishes a dry-run tarball (cargo package --list / --dry-run) and diffs repo source vs the published 0.3.0 artifact to detect drift — the publish workflow only gates the moment of release, not after.  _(unknown, 2026-10-12)_
