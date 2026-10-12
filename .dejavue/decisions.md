# Decisions


## 2026-10-11T12:47:18-05:00 — mustang gpu: use bliss-anyrender-vello 0.3 fork instead of upstream anyrender_vello 0.7

Reason:
Share one VelloScenePainter/VelloWindowRenderer type with bliss and cut upstream ties. MSRV stays 1.88: bliss-anyrender-vello 0.3.0 and 0.3.1 are source-identical (only rust-version metadata differs), so the MSRV-aware resolver picks 0.3.0 on 1.88. PR nixpt/mustang#3.

Author type: agent

