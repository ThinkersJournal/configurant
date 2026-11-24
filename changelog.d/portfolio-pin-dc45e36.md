kind: CI: portfolio workflow pin moved to CodeRipper dc45e36
---
The `portfolio` job in `.github/workflows/ci.yml` now calls the CodeRipper portfolio workflow at
`dc45e3632ab9f471292458e5ccba5f375084a70a` (was `c72a6e0`, 0.47.5), as pm-ants asked on 2026-11-24. The lock already holds
`url` 2.5.9 (RUSTSEC-2026-0592) and has no `cookie` entry (RUSTSEC-2026-0594), so no lock line changes and no
advisory is ignored. Verified by the portfolio check's result on this PR.
