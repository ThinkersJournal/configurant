# Patent sweep: configurant 0.5.x @ ad5700c

Date: 2026-10-25; Run by: the configurant lane, Claude Sonnet 5.5; Method: AI-assisted search and read; NOT legal advice; NOT a freedom-to-operate opinion.

Covers the 0.5.x line (swept at `ad5700c`, version 0.5.0: the core crate, `configurant-secrets`, `configurant-tui`, `configurant-gui`, `configurant-scan`). Process: `C:\Projects\PATENT-SWEEP-PROCESS.md` (CireSnave, 2026-10-25, board 286). Re-sweep when a new family of capability is added, and by 2027-10-25.

## Ideas swept

What the project does that might be new or unusual (mechanisms, not features):

1. A schema file (TOML, no generated code) declares typed keys with defaults, ranges, choices, env name and flag; a loader layers default, file, environment and command line over it, validates every value, and returns all errors in one load rather than the first.
2. The loader keeps, for each key, the chain of layers that set it, and `explain(key)` lists each layer lowest first and marks the one in force.
3. A value the schema marks `secret` is accepted only from the environment or a declared source, has no default or flag, and is replaced by `<redacted>` in every error, `Debug`, `Display`, `explain` and `help`.
4. The schema holds a reference to a secret (a name, never a value) as `source = "<source>:<reference>"`; a separate crate fetches it through a pluggable source trait at load time, the environment wins, and the fetched value is wiped on drop.
5. A `file` secret source reads a bare file name from an operator-named secrets directory, refusing path traversal, symlinks that leave the directory, files readable by group or others, and oversize or non-UTF-8 content.
6. A value with a `provider` is validated, and its choices listed, by a pluggable module registered by name; a schema default is checked by that module at load.
7. A module's answer to a changed value is a `Plan`, data only; a module never acts, and a front end shows the plan and executes it only after the user confirms.
8. A storage-backend choice is made from declared needs (capability identifiers that must hold plus an order of importance): the module returns the backends a build offers, ranked, each with a reason; a changed pick yields a data-transfer plan.
9. A configuration file migrates between format versions by steps made only of rename, one-to-one value map and a lossless drop (which removes a key only while it holds exactly an expected string), so a step can be carried back; the edit keeps comments and layout and reports key names only.
10. An edit session stages changes through the same checks as a load, and a review value is the only way to a confirmed value that alone yields new file text and the plan to run (a front end cannot write without the confirmation step).
11. An unknown key from a file, the environment or the command line is an error carrying the nearest valid key as a suggestion.
12. A text-level check reads application source for literal key names passed to the library's accessors and reports those the schema does not define.
13. Scan assist: references to configuration reads are found through a language server and turned into proposed schema entries.
14. Integer bounds on a value are compared exactly as integers, and the CLI, TUI and GUI front ends are driven from the one schema.

## Queries (source, query, date)

All run 2026-10-25 against Google Patents (`patents.google.com`, through a web search tool restricted to that domain; results are search-engine ranked, not patent-office query syntax). Closest hits were then opened and their claims read (see Hits).

| # | idea | query |
|---|---|---|
| Q1 | 1, 2 | configuration schema validation layered sources environment command line precedence report provenance of each value patent |
| Q2 | 3 | secret value redacted from error messages logs configuration schema marks field secret patent |
| Q3 | 4 | configuration references secret by name resolved at load time from secret store pluggable source patent |
| Q4 | 7 | module returns plan of actions as data executed only after user confirmation configuration change storage backend migration patent |
| Q5 | 8 | select storage backend from declared capability requirements ranked list with reasons configuration patent |
| Q6 | 9 | reversible configuration file migration rename keys map values preserve comments lossless carry back patent |
| Q7 | 11 | unknown configuration key detected suggest closest valid key edit distance typo configuration file validation patent |
| Q8 | 13 | analyze source code find configuration parameter reads language server references generate configuration schema automatically patent |
| Q9 | 10 | staged configuration edits validated then confirmation produces committed configuration file and action plan patent |
| Q10 | 5 | secret read from file refused if group or world readable or symlink outside directory secrets directory mounted container patent |
| Q11 | 2 | hierarchical configuration merge default file environment variable command line override which source supplied value explain patent |
| Q12 | 10 | typed confirmation token required before applying changes, change set cannot be applied without review step, software configuration patent |
| Q13 | 12 | static analysis of program source detects configuration keys not defined in configuration schema build-time check patent |
| Q14 | 14 | command line interface generated from schema flags environment variable names help text generated from declared options patent |

Idea 6 (a module validating a value) is covered by Q4, Q5 and the US8073935B2 / US7529818B2 reads; idea 14's exact-integer comparison is a numeric-comparison detail with no search of its own.

## Hits

Claims were read from the Google Patents text (claim 1 of US8073935B2, US7529818B2, US6725367 and US8762406B2 read in full from the page; the others from the page's independent claims). Legal status is Google's own label and is "an assumption, not a legal conclusion".

| # | patent | title | assignee | status | claim(s) read | overlap | our difference | verdict |
|---|---|---|---|---|---|---|---|---|
| 1 | US8073935B2 | Pluggable semantic verification and validation of configuration data | Sun, now Oracle America | active, expires 2029-07-01 | indep. 1, 17, 34, 49 | A change request for one configuration element is checked by a per-element test case before it is applied (ideas 6, 10). | Every independent claim requires a plurality of servers, a persistent store, an in-memory copy at one server, validation against that copy "independent of the persistent store", and notification to registered servers to synchronise. We have no servers, no replicated in-memory copy and no notification; we validate a value against a schema and a module. | clear |
| 2 | US7529818B2 (US20050262225A1) | System and method for performing validation of a configuration | BEA, now Oracle International | active, expires 2027-06-28 | indep. 1, 10, 19, 20 | Validate a set of changes before accepting them and signal acceptance or rejection (ideas 1, 10). | Claim 1 requires a managed server receiving a configuration file update, comparing it "to a plurality of managed server configuration MBeans in an MBean graph" tied to "a running state of the managed server"; claim 20 is bean-tree differencing. We have no managed server, no MBean graph and no running-state comparison. | clear |
| 3 | US8762406B2 | Real-time data redaction in a database management system | Oracle International | active, expires 2032-01-08 | indep. 1, 11 | Redaction of marked data before it is returned (idea 3). | Claim 1 is a database server redacting selected rows under a policy "mapped, via a stored mapping, to an identifier of the client" and stored in the database's data dictionary. We redact a schema-marked value in errors and output of a library; no database, client identifier or dictionary. | clear |
| 4 | US11303541B2 | Systems and methods for transaction models and declaration configuration in a distributed architecture | Ciena | active, expires 2039-02-14 | indep. 1, 9, 17 | Changes are gathered and staged before one commit (ideas 7, 10). | Claim 1 is a service forwarding a staging request to peer services and recursing until all provisioning is collected, then distributing an amended request. We have one process, no peer services, no recursion. | clear |
| 5 | US7545931B2 | Protection of application secrets | Microsoft | expired (lifetime), 2024 | indep. 1, 14, 20, 23, 26, 29, 31, 32 | A stored secret is found through a "reference value" (claim 32; idea 4). | Claim 32 resolves the reference to a location in an encrypted store and decrypts with a key hashed from machine entropy and the application's evidence. We resolve a name through a pluggable source, with no machine-entropy or evidence-derived encryption. Also expired. | clear |
| 6 | US7117482B2 | Migration of configuration data from one software installation through an upgrade | Sony | expired (lifetime), 2024 | indep. 1, 9, 16 | Settings carried across versions by a map (idea 9). | Claims store values in non-volatile memory keyed to NVRAM addresses by a map file and apply only when the new version number is higher; no claim covers renaming, reverse migration or comment preservation. We rename, map and drop by named steps, reversible. Also expired. | clear |
| 7 | US10956037 | Provisioning storage allocation using prioritized storage system capabilities | IBM | lapsed 2025-03-23 for unpaid maintenance fee (Google: expired, fee related; reinstatement not checked) | indep. 1, 8, 15 | Choose a storage system whose capabilities meet conditions, with a sorted list offered to the user (idea 8). | Claims need "all mandatory capabilities" met and a capability or weighted score, over conditions written as attribute, comparison and value. No reason attached to each entry, no module or plugin interface, no capability identifiers. Ours: identifier needs, module-returned list, a reason per entry. | clear |
| 8 | US6725367 | Methods and systems for constructing and interpreting hierarchical configuration files | Microsoft | expired (lifetime), 2022-10-06 | claim 1 in full; indep. 1, 8, 9, 15, 16, 21, 22, 27, 28, 32 | A parameter's value is taken from the highest-precedence section that defines it (idea 1). | Claims are about sections of one initialization file, or merging initialization files, by a precedence rule. No environment or command-line layer is an input, no reporting of which file supplied a value, no schema validation. Also expired. | clear |
| 9 | US7693967 | Method and apparatus for assisting system configuration activities | Ericsson | lapsed 2022 (fees); Google adjusted expiry 2029-02-04 is superseded | indep. 1, 9 | Nearest valid item by edit distance (idea 11). | Claims require an invalid configuration word that encodes "a detected combination of installed system components", in a wireless node configured by circuit cards. We suggest a key name from a schema. | clear |
| 10 | US6907572B2 | Command line interface abstraction engine | Occam, now Calix | expired (lifetime), 2022-09-27 | indep. 1, 12, 23 | A CLI is defined by description files and help text comes from them (idea 14). | Claims map CLI commands to system object/method actions through XML description files compiled to a run-time module. No environment binding, layered values or secrets. Also expired. | clear |
| 11 | US7058582 | Method for performing programming by plain text requests | IRN | lapsed 2010-06-06 (fees; Google: expired, fee related) | indep. 1, 8 | An approval step (confirmation e-mail) before a change goes live (idea 10). | Both independent claims are an emailed request that modifies a web site. | clear |

Opened only by title and search snippet, not claim-read, and judged unrelated by subject (so no verdict is claimed for them): US20040019670A1 (the application that became hit 1), US20050229097A1, US7821667B2, US20090006943A1 (schema or document validation in other domains), US20200076777A1, US12367314B1, US8086615B2 (data encryption or redaction in databases and message transport), US9165153B2, US9355277B2, US20190065764A1 (secret stores in other settings), US8806456B2, US10726040B2 (source-code and database-table round trips), US20120167037A1, US20110078667A1, US7340726B1 (static analysis in other domains). For ideas 5 and 12 the searches found no patent with a matching mechanism.

## Needs review (to the PM)

None. Every claim-read hit is `clear` on its independent claims, with the reason in the table. No design change was made as a result of this sweep.

## Limitations

- AI-assisted: a language model chose and ran the queries and read the claims. It can miss a patent, mis-rank a hit or misread a claim; the claim text of hits 1 to 3 and 8 was read in full on the page, the rest from the page's independent claims.
- Coverage: one search engine's view of Google Patents, 14 queries, 2026-10-25. Espacenet, WIPO PATENTSCOPE and the USPTO's own search were not queried directly, and no non-English or non-US family was searched except incidentally. No search by assignee or by CPC class was run.
- Legal status and expiry are Google's labels. A lapsed or expired patent can have family members or continuations that are in force; none were checked.
- The idea list was written by the lane that owns the project and may omit a mechanism.
- This is a due-diligence read. It is not legal advice and not a freedom-to-operate opinion; a lawyer's review is a separate, later purchase.
