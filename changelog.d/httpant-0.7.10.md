kind: httpant 0.7.10, refused requests reliably deliver their 403
---
`configurant-web` now needs httpant 0.7.10 (was 0.5.19). A refused request whose body was never read (a foreign `Host`, a bad origin) now reliably gets its 403: httpant drains the unread body, up to 8 MiB, before it answers, where it used to lose the answer to a TCP reset on Windows (configurant#111, httpant#70). The guard test covers 50 B, 16 KB, 70 KB, 1 MB and 5 MB bodies, strict 403 each. — lane: configurant
