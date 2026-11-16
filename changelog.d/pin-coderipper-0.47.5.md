kind: CI: portfolio workflow pinned to CodeRipper 0.47.5
---
The `portfolio` job now calls `ciresnave/coderipper/.github/workflows/portfolio.yml` at `c72a6e03479d914de8ea8a73609ba73e93a0de92` (CodeRipper 0.47.5, was 0.9.24). `.coderipper.toml` already uses current rule ids (`SUP-002`, `SEC-006`), so `tools/migrate-rule-ids.py --dry-run` reports nothing to migrate. No code change. Verified by reading the Portfolio job of this PR.
