# Patent sweep: configurant 0.5.x @ ad5700c

Date: 2026-10-25; Run by: the configurant lane, Claude Sonnet 5.5; Method: AI-assisted search and read; NOT legal advice; NOT a freedom-to-operate opinion.

Covers the 0.5.x line: the first sweep below at `ad5700c` (version 0.5.0), the addendum for rows 20 to 26 (0.5.7 to 0.5.15, 2026-10-29), the addendum for row 28, the plan executor (0.5.17 to 0.5.21, 2026-10-30), and the last addendum for row 11, the ports, logging and paths modules (0.5.25 to 0.5.31, 2026-10-30). First sweep: the core crate, `configurant-secrets`, `configurant-tui`, `configurant-gui`, `configurant-scan`. Process: `C:\Projects\PATENT-SWEEP-PROCESS.md` (CireSnave, 2026-10-25, board 286). Re-sweep when a new family of capability is added, and by 2027-10-25.

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


---

# Addendum 2026-10-29: rows 20 to 26 (0.5.7 to 0.5.15)

Date: 2026-10-29; Run by: the configurant lane, Claude Sonnet 5.5; Method: AI-assisted search and read; NOT legal advice; NOT a freedom-to-operate opinion. Order from the PM, 2026-10-29: re-sweep the new capability families, say "outside, as read" and never "clear", list what was not read.

Swept at `9e61489` plus the wizard branch head `85de2cc` (PR #54, 0.5.15): `src/view.rs`, `src/wizard.rs`, the schema's `prompt`/`example`/`hint`/`section`/`advanced`/`[[step]]`, `Schema::parse_with` and `Mount`. This addendum covers the 0.5.x line up to 0.5.15; the sections above cover 0.5.0. Re-sweep when a new family of capability lands (the next candidates are the plan executor, row 28, and a browser renderer, row 29) and by 2027-10-25.

Verdict words in this addendum: **outside, as read** means no independent claim that was read covers what the project does, with the reason; **overlap, designed around** and **needs review** are as in the process note. None of "outside, as read" is a statement that the patent is not infringed.

## Ideas swept (new since the sections above)

15. A value in a schema carries a plain-language question, a sample answer and a one-sentence hint; the sample answer is checked by the same type, range and choice rules as a default, and a secret may not have one.
16. A library ships its own options as a schema fragment (keys relative to a mount, behind a Cargo feature) and an application mounts it under a name; environment and flag names derive from the mount, the application may restate a mounted value only to change its default and wording, and a mounted secret stays secret.
17. An editing session is exposed as plain serialisable data (every field with question, example, hint and choices, plus a review of what would change) and a small JSON event set (set, unset, confirm), so a renderer on any platform drives the one validated session; a secret appears only as a placeholder and cannot be set; a refused event changes nothing.
18. Fields are grouped for a form into sections derived from the first segment of the key (or a declared section name), a value can be flagged advanced, and tables and maps that only a file can set are listed apart.
19. A wizard's steps are declared in the schema (or derived, one per section); each step may be hidden by a data-only condition (equals, not equals, is set) on another value; next and back skip hidden steps; goto returns only to steps already reached; confirmation works only at a final review.
20. A saved wizard position holds step identifiers only, never a value, and resuming from an identifier the schema no longer has lands on the first step with a note.

## Queries (source, query, date)

All run 2026-10-29 against Google Patents (`patents.google.com`, through a web search tool restricted to that domain; results are search-engine ranked, not patent-office query syntax). One search per query; the closest hits were then read with the shared tool `patent-claims.py`.

| # | idea | query |
|---|---|---|
| Q15 | 19 | wizard step conditional display next back steps defined by declarative schema configuration user interface patent |
| Q16 | 17 | platform-independent user interface description serialized view model events rendered by different client platforms form generated from schema patent |
| Q17 | 16 | library supplies its own configuration options schema merged into application configuration namespace prefix mount patent |
| Q18 | 15 | configuration parameter definition includes prompt text example value help hint displayed to user when entering setting validated patent |
| Q19 | 18 | settings grouped into sections automatically from hierarchical key names advanced settings collapsed form generation patent |
| Q20 | 20, 19 | installation wizard resume saved position step identifiers without values skip steps whose condition not met patent |
| Q21 | 15, 17, 18 | user interface generated automatically from data schema with validation constraints example values and field groups patent claims |
| Q22 | 17 | secret field masked in generated form view model sent to client never includes secret value patent |

## Hits

Independent claims were read in full from the Google Patents text (the shared tool). Status is Google's own label ("an assumption, not a legal conclusion").

| # | patent | title | assignee | status | claim(s) read | overlap | our difference | verdict |
|---|---|---|---|---|---|---|---|---|
| 12 | US8869048B2 | Declarative and multi-mode wizard framework | Microsoft | active, expires 2033-04-03 | indep. 1, 12, 17 | The closest hit. A wizard engine reads a declarative description (a "story") with data items of a type and name and UI instructions, and generates a user interface from it, one input component per data type (ideas 17, 19). | Every independent claim requires a user-created story holding "an action to perform defined by the user", a data collection object, and "at least one section for defining user interface instructions"; the engine then generates and presents the user interface, causes the action to be performed from the populated object, and "modif[ies] said user interface based on ... a data item layout hint, a section item layout hint, and a story layout hint". Our library generates and presents no user interface (it returns data to renderers), a plan is module data and never an action in the description, and there are no layout hints in the schema. Recheck if layout hints, or a library-generated interface, are ever added. | outside, as read |
| 13 | US9009585B1 | Software wizard interface | Avaya, now Extreme Networks | active, expires 2031-11-14 | indep. 1, 13, 20 | A wizard defined by a stored description file that links to separately created forms, order and content by condition (idea 19). | Claim 1 and 13 need forms created before the wizard, selected from a list, customised by delete/modify/add field instructions, and a stored file holding links to them; claim 20 displays different form lists by a user attribute. Our steps name values of one schema, nothing is linked or customised from a forms list, and no user attribute selects steps. | outside, as read |
| 14 | US8407610B2 | Executable and declarative specification for graphical user interfaces | SAP Portals Israel | active, expires 2031-02-13 | indep. 1, 7, 13 | A declarative abstract representation of a GUI that is independent of the runtime platform, from which GUIs for different platforms are made (idea 17). | Needs a model representation from a modeling tool with GUI components and dependencies, three sets of mapping rules, and automatic generation of a first and a second GUI from the abstract representation. We have no modeling tool, no mapping rules and generate no GUI; each renderer is hand-written over the data. | outside, as read |
| 15 | US7424485B2 | Method and apparatus for generating user interfaces based upon automation with full flexibility | Microsoft | expired (lifetime), 2024-11-03 | indep. 1 | A display-target-independent logical form generated from an application model, mapped to display targets on different platforms (idea 17). | Needs a selected logical form type, a first map from model property types to logical controls, and mapping to a display target per platform with independent layouts. We do not map to display targets. Also expired. | outside, as read |
| 16 | US12212464B2 | Generating user interface elements based on data schema files | Nintex | active, expires 2042-04-29 | indep. 1, 16 | A UI is built from a data schema (ideas 15, 17). | Both independent claims are about testing "candidate sentinel values" against the schema with a validation service, receiving validation messages, and building elements from the compliant values. We validate one typed value at a time and generate nothing from sentinel testing. | outside, as read |
| 17 | US8560941B2 | Schema based user interface mechanisms | Microsoft | expired, 2031-09-08 (Google: expired, fee related) | indep. 1, 9, 15 | A schema with a configurable parameter and limits drives a UI tool bound to a data object (ideas 15, 17, 18). | Needs a rule template with a limit, selecting a UI tool from a library by the rule, creating and binding a data object to the tool, and a rule library of generic rules chosen by rule type. We select no UI tool and bind no UI object; the data is returned to a renderer. | outside, as read |
| 18 | US7096465B1 | Process control configuration system with parameterized objects | Invensys | expired (fee related) | indep. 1, 9, 11, 19, 22, 30, 32, 39, 42 | Parameters carry attributes, and claims 19 and 39 group parameters for presentation and editing (idea 18). | Every independent claim is objects that descend from other objects and inherit parameters so that an ancestor change takes effect in the descendant without recompilation, for a control system. We have no object descent and no inheritance; sections come from the key name. Also expired. | outside, as read |
| 19 | US7219332B2 | Configuring software components with transformation component using configurable and non-configurable data elements | Microsoft | expired (lifetime), 2022-08-19 | indep. 1, 9, 13, 17, 22, 23 | A configurable module supplies configuration options that are merged into a target data set (idea 16). | Needs configurable and non-configurable data elements and transformation instructions from the module applied to install the elements into a target data set. A fragment of ours is a schema, with no transformation instructions and no installation into a target; it is mounted under a name. Also expired. | outside, as read |
| 20 | US7757234B2 | Methods and software for a batch processing framework for wizard-based processes | SAP | active, expires 2029-05-04 | indep. 1, 6, 8, 13 | A wizard with steps, a step type that decides whether user input is needed, resumed after an interruption (ideas 19, 20). | Needs steps typed confirmation, processing, optional or mandatory input, execution as a batch process by a batch job manager, interruption at a step by comparing its type with a user parameter, and an augmented user interface resuming the batch. We have no step types, no batch execution and no job manager; our resume restores a position by identifier. | outside, as read |
| 21 | US6920631B2 | Software installation and validation using custom actions | Microsoft | expired (fee related), 2019-09-11 | indep. 1, 7, 13, 18, 24, 25 | Actions read from a table, skipped by a condition (idea 19). | Every independent claim is a database with a custom action table whose rows an installation engine reads and executes. We execute nothing and have no action table. Also expired. | outside, as read |
| 22 | US20110093779A1 | Wizard in a Wizard Engine | Ericsson | active, expires 2029-05-06 | indep. 22, 33 | A wizard interrupted, its data stored, and later resumed (idea 20). | Claims manage a web application of at least two wizards: on a request for data held in another wizard, interrupt the current one, store its data, invoke the other and resume the first. We have one wizard, store no data in a saved position (identifiers only), and invoke no second wizard. | outside, as read |
| 23 | US6871340B1 | System and method for generating software tools | Lucent | expired (fee related) | indep. 1, 29, 45 | A wizard created from stored steps and selections (idea 19). | Claims are a wizard-authoring method that prompts for input process steps and their potential selections, displayed concurrently, and stores them. We do not author a wizard by prompting. Also expired. | outside, as read |
| 24 | US7895522B2 | Layout of platform specific graphical user interface widgets migrated between heterogeneous device platforms | NTT Docomo | active, expires 2027-08-31 | indep. 1, 9, 16, 23 | A platform-independent presentation model for different devices (idea 17). | Every independent claim is splitting a widget across pages when it does not fit a device's display. We do no layout. | outside, as read |
| 25 | US8775478B2 | System and method for generating automatic user interface for arbitrarily complex or large databases | individual inventor | expired (fee related) | indep. 1 (the only independent claim shown by the tool) | A UI generated from database metadata with validation (idea 15). | Claim 1 is a hierarchical context stack that suspends a table's working state while drilling down a relationship in a relational database. We have no database and no drill-down state. Also expired. | outside, as read |

## Found but not read (no verdict claimed)

Opened only by title and snippet from the search results, because their subject is outside the ideas above or they are older than the project's mechanisms; no verdict is claimed for them and a read is owed if the project grows toward them: US5339432 (device-driver configuration with PROMPT and HELP statements; the oldest, likely expired), US20070240150A1 (resuming an aborted multi-product installation), US9047105B2 (configuration modelling by merging extension objects), US8959194B1 (merging network device configuration schemas), US9319270B2 (configuration of third-party apps in a sandbox), US10509660 (assessing configuration profiles for a user configurable device), US7299410B2 (reporting hierarchically arranged data in markup, expandable views), US11373034 and US11875110 (dynamic form generation; form sections shown collapsed by default), US7017145B2 (model and view separation), US20060047780A1 (schema-driven server and client with serialised data objects), US20140074891A1 (UI from a relational data model), US8204929B2 (hiding sensitive information in a form field), US20170041302A1 (shared-secret initialisation), US20060005001 (hierarchical INI files). For idea 17's redaction of a secret in a view and idea 20's identifiers-only position, the searches found no patent with a matching mechanism.

## Needs review (to the PM)

None. Every hit that was read is outside, as read, with the reason in the table. Hit 12 (US8869048B2, active to 2033) is the closest; the note in its row says when to read it again. No design change was made as a result of this addendum.

## Limitations

- AI-assisted: a language model chose and ran the queries and read the claims. It can miss a patent, mis-rank a hit or misread a claim. The tool's text is machine-converted. US8775478B2 showed only claim 1 as an independent claim.
- Coverage: one search engine's view of Google Patents, 8 queries, one search each, 2026-10-29. Espacenet, WIPO PATENTSCOPE and the USPTO's own search were not queried directly; no non-English or non-US family was searched except incidentally, and no search by assignee or CPC class was run (for example G06F 9/451 and G06F 8/38 for generated user interfaces and wizards).
- Legal status and expiry are Google's labels. A lapsed or expired patent can have family members or continuations that are in force; none were checked.
- The idea list was written by the lane that owns the project and may omit a mechanism. The view-model and wizard are libraries, not user interfaces; a renderer built on them (the TUI, the GUI, a future browser page) is a different thing to sweep, and the browser renderer (row 29) will need its own.
- This is a due-diligence read. It is not legal advice and not a freedom-to-operate opinion; a lawyer's review is a separate, later purchase.

## Addendum for row 28, the plan executor (0.5.17 to 0.5.21, 2026-10-30)

Covers `configurant::exec` as built in 0.5.19 (steps, validation, hash, inert `describe`), 0.5.20 (`run`, state file, `Host`, Windows ACL rule) and 0.5.21 (`undo`, `Undo::Remove`), at `daa0fd1`. Run by the configurant lane, Claude Sonnet 5.5. Wording as above: "outside, as read"; not legal advice.

### Ideas swept (new since the sections above)

21. A plan of steps (make a directory, write a file, run a command, control a service) is shown to the user as data, and execution is refused unless the caller presents the hash of the exact plan that was shown; showing the plan changes nothing.
22. A plan that contains any known secret value anywhere (title, path, argument, environment literal, inline content) is refused at construction; a secret is resolved only at run time and goes only to a child environment variable or a file created owner-only, and a state file, report or error never holds a value.
23. On Windows a file that carries a secret is written only if the host reports an owner-only access list; otherwise the step is refused, never downgraded.
24. A run records each step as pending, applied, verified or failed in a state file that holds the plan hash and step identifiers only; a re-run skips verified steps and resumes at the first failure, and a state file for another plan is refused.
25. A command is an argument array run with no shell and no stdin and killed at a timeout; every path is checked again against the real filesystem (symlinks and junctions followed) and must stay inside the roots the plan declared.
26. Undo reverses the steps that ran, last first, and removes a file only while it still holds exactly what the step wrote, a directory only while it is empty, and only a path the step itself made; what cannot be undone is reported, and the run goes on.

### Queries (source, query, date)

All run 2026-10-30 against Google Patents (through a web search tool restricted to `patents.google.com`; results are search-engine ranked). One search per query; the closest hits were then read with `patent-claims.py`.

| # | idea | query |
|---|---|---|
| Q23 | 21 | installation plan hash confirmed by user execution refused if plan differs from displayed plan declarative steps |
| Q24 | 22, 23 | secret credential provided to child process environment variable or file with owner-only permissions never stored in script or log deployment steps |
| Q25 | 26 | rollback installation undo removes file only if unchanged since installation content hash matches directory removed only if empty |
| Q26 | 24 | resumable deployment state file records step status pending applied verified failed skip completed steps idempotent re-run |
| Q27 | 25 | command execution argument array no shell interpretation path confined to declared directories symlink resolved real path check before write |
| Q28 | 21, 22 | dry run preview of changes create overwrite unchanged per step configuration management plan before apply secrets redacted |

### Hits

Claims were read with the shared tool; where the tool output was cut short, only the independent claims that were shown are listed as read. Status is Google's own label.

| # | patent | title | assignee | status | claim(s) read | overlap | our difference | verdict |
|---|---|---|---|---|---|---|---|---|
| 26 | US10620933B2 | Techniques for efficient application configuration patching | Oracle | active, expires 2037-01-03 | indep. 1 (8 begins alike; its end was cut) | The closest hit. A configuration patching execution plan of steps (ideas 21, 24); dependent claims 3, 4 and 7 drop steps found in an inventory of previously executed steps and record each executed step (idea 24). | Claim 1 needs binary patches plus patch metadata declarations that say how to modify configuration data, and a plan with an online phase and an offline phase, each running binary patching and configuration patching on a server. We patch no binaries, have no online and offline phases, and our state file is bound to a plan hash and holds step status, not an inventory of patches. Recheck if we ever apply binary patches or split a run by whether a server is up. | outside, as read |
| 27 | US8719809B2 | Point in time rollback and un-installation of software | Commvault | expired (fee related), 2031-09-13 | indep. 1 (6 was cut) | Rollback of an update, files restored to a prior state (idea 26). | Claim 1 intercepts update commands, copies the files before they are updated, keeps a rollback data structure with storage locations and copy times, and restores by a point-in-time request. We copy nothing; undo removes what a step created, and only while it still holds what the step wrote. Also expired. | outside, as read |
| 28 | US5857204A | Restoring the state of a set of files | Ab Initio | expired (lifetime) | indep. 1, 2 (9 was cut) | Perform, finalize and undo routines for file operations, with a transaction log that rollback reads (idea 26). | Claims 1 and 2 are a library of routines that stand in for native file operations and preserve what is needed to roll them back, with a transaction master and a log naming the finalize and undo routines. We wrap no native operation and keep no preserved copy; a step is data checked by hash. Also expired. | outside, as read |
| 29 | US9348738B2 | Testing functional correctness and idempotence of software automation scripts | IBM | expired (fee related) | indep. 1 | Idempotence of an automation script that puts a system into a target state (idea 24). | Claim 1 is a testing module that builds a state transition graph from a script and runs test cases to decide whether the script is idempotent or convergent. We test no script; each step checks for itself whether it is done. Also expired. | outside, as read |
| 30 | US20130167200A1 | Techniques to store secret information for global data centers | Microsoft | active, expires 2032-07-21 | indep. 1, 9 (16 was cut) | A secret generated and used by a service rather than passed in a request (idea 22). | Claims are a front end service that generates a secret for a data center, keeps it on back end storage the data center cannot reach, and uses it for a validated request. Our executor generates no secret, runs no front end or back end service and stores none. | outside, as read |
| 31 | US7051211B1 | Secure software distribution and installation | IBM | expired (fee related), 2023-05-19 | indep. 1 | A secret used while installing (idea 22). | Claim 1 is a server and client holding two secret values from which a key decrypts the software to install. We decrypt nothing and hold no installation keys. Also expired. | outside, as read |

### Found but not read (no verdict claimed) for row 28

Opened only by title and snippet from the results: WO2005106654A1 (installation from removable media with signed metadata and per-file hashes), US7003672B2 and US9223967B2 (hash checks of program code before execution), US20090006260A1 and US8266062B2 (licensing hash that refuses installation on mismatch), US7434259B2 (prompt before installing an unsigned application), US8978160B2 (selective software rollback), EP3039534A1 (idempotent workflow), US9589042B1 (status flag to avoid repeated work), US9996331 and US12632240 (deployment status), US7797534B2 (controlled path-based process execution), US9553855B2 and US7545931B2 (keys and secrets kept for the owning process), US8161544B2 (security descriptor on a child process), US9092576B2 (dry runs with roll-back). Their subjects differ from the ideas above or the snippet showed no matching mechanism; a read is owed if the executor grows toward them. For idea 21's hash-bound confirmation, idea 25's real-path check and idea 23's Windows owner-only rule, the searches found no patent with a matching mechanism (a null from one search each, not a clearance).

### Needs review (to the PM) for row 28

None. Every hit read is outside, as read, with the reason in the table. Hit 26 (US10620933B2, active to 2037) is the closest; its row says when to read it again. No design change was made.

### Limitations for row 28

Same as above: AI-assisted, one engine, 6 queries with one search each, 2026-10-30; Espacenet, WIPO and the USPTO were not queried and no CPC-class search was run (for example G06F 8/60 for software installation, G06F 21/60 for protecting data). Several independent claims were cut off in the tool output and were not read (marked in the table). Status and expiry are Google's labels; continuations and family members were not checked. Not legal advice and not a freedom-to-operate opinion.

## Addendum for row 11, the ports, logging and paths modules (0.5.25 to 0.5.31, 2026-10-30)

Covers `configurant::modules::{ports, logging, paths}` and the module interface additions they brought (`Module::validate_for`, `Module::validate_all`, interface version 2, the schema field `privileged`), at `c864714` (0.5.31). Run by the configurant lane, Claude Sonnet 5.5. Wording as above: "outside, as read"; not legal advice.

### Ideas swept (new since the sections above)

27. A port in a configuration file is refused if it is below 1024 unless the schema entry for that key declares, with a stated reason, that a privileged port is intended; the reason is shown by `explain` and `help`.
28. Two port values in one configuration are refused if they bind the same port on overlapping hosts (the same host without regard to case, or either side a wildcard, or a bare port that means every host); the check runs once per load over the winning value of every such key, and each key at fault is told which other key it collides with, never a value.
29. A path value is checked by text alone, under a chosen platform's rules: not empty and free of control characters, kept under an application-chosen root after `.` and `..` are resolved (whole components, case folded on Windows), required relative or absolute, free of Windows reserved device names, trailing dots and spaces and drive-relative forms, and, for a file, not ending in a separator; no file, link or drive is looked at.
30. A log level or filter (`target=level` directives with at most one default) is checked for grammar, and an application can set a ceiling that refuses any level more verbose than it, in a bare level and in every directive of a filter.
31. When a directory-valued setting changes, the module returns a plan holding one make-directory step for the new value and never a step that removes or moves the old one; a front end shows the plan and runs it only after confirmation.
32. A validator is a named module that a schema entry points to, registered by the program; the module interface carries a version number, and a module written for an older version still registers.

### Queries (source, query, date)

All run 2026-10-30 against Google Patents (through a web search tool restricted to `patents.google.com`; results are search-engine ranked). One search per query; the closest hits were then read with `patent-claims.py`.

| # | idea | query |
|---|---|---|
| Q29 | 27 | configuration value validation port number below 1024 privileged refused unless schema declares reason |
| Q30 | 28 | detect conflicting network port assignments between configuration settings same port overlapping wildcard address validation before deployment |
| Q31 | 29 | path validation without filesystem access normalize dot dot confine to root directory reserved device names operating system rules |
| Q32 | 30 | log level filter configuration validation maximum verbosity ceiling refuse trace level production setting |
| Q33 | 31 | configuration setting change produces plan to create new directory old directory not removed or moved plan shown to user for confirmation before execution |
| Q34 | 32 | pluggable validator module registered by name for configuration value schema provider interface version cross-key validation |

### Hits

Claims were read with the shared tool; the independent claims of each hit were shown in full unless noted. Status is Google's own label.

| # | patent | title | assignee | status | claim(s) read | overlap | our difference | verdict |
|---|---|---|---|---|---|---|---|---|
| 32 | US20040019670A1 | Pluggable semantic verification and validation of configuration data | Sun Microsystems (now Oracle America) | active, expires 2029-07-01 | indep. 1, 18, 36, 53 | The closest hit. A test case per configuration element decides whether a change to that element is valid, and the change is applied only if it is (ideas 27, 28, 32; also the interactive edit session). | Every independent claim needs a server holding an in-memory representation of the configuration that is formatted differently from the persistent store, a change request received from a process, and the server applying the change. We have no server and no second representation: a load is a pure function of its input texts, a module judges a value and applies nothing, and a check across keys runs once per load. Recheck if ConfigurAnt ever runs as a daemon that accepts live change requests from other processes and applies them to a held configuration. | outside, as read |
| 33 | US7287077B2 | Reservation of TCP/UDP ports using UID, GID or process name | IBM | expired (lifetime), 2025-12-08 | indep. 1, 10, 18 | Ports and who may bind them (idea 27). | The claims enable a bind only for a pre-authorized user or application and reserve the port until its process completes. We bind nothing and reserve nothing; we refuse a value in a text. Also expired. | outside, as read |
| 34 | US6026402A | Process restriction within file system hierarchies | Hewlett Packard | expired (lifetime) | indep. 1, 9 | Confining something to a sub-hierarchy (idea 29). | The claims change the host system so a process cannot access or address objects outside an assigned sub-hierarchy, and carry the restriction to its child processes. We confine no process; we compare the text of a path with a root. Also expired. | outside, as read |
| 35 | US7619989B2 | Routing configuration validation apparatus and methods | Alcatel Lucent | expired (fee related), 2027-10-22 | indep. 1, 13 | Detecting a misconfiguration by comparing assigned and used addresses (idea 28). | The claims passively collect routing information from routing protocol messages and compare it with which addresses were assigned. We read no network traffic; we compare two values in one configuration. Also expired. | outside, as read |
| 36 | US9552274B2 | Enhancements to logging of a computer program | VMware | expired (fee related), 2034-12-29 | indep. 1, 9, 16 | Logging (idea 30). | The claims rank methods in source code by importance and recommend logging changes from a comparison with an ideal method. We check a configured level or filter and read no source code. Also expired. | outside, as read |
| 37 | US20040204949A1 | Method and system for implementing group policy operations | Individual | expired (fee related) | indep. 1, 29, 32, 39, 46, 53, 63, 69 | A change that is rolled back when it fails, and a backup before change (idea 31). | The claims are about group policy objects: restoring a second object's state after a failed operation, backing up, migrating between domains, copying, canceling. Our module plan holds a make-directory step and no rollback of its own, on a configuration value. Also expired. | outside, as read |

### Found but not read (no verdict claimed) for row 11

Opened only by title and snippet from the results: US20090271863A1 (identifying unauthorized privilege escalations), US20100049876A1 and US7634584B2 (packet validation, virtual network interface), EP1358559A4 and US7290283B2 (network port profiling), US6795918B1 (service level computer security), US7305549B2 (filters to isolate untrusted ports of switches), US7821667B2 (validation of print configuration documents against a schema), US7542957B2 (rich web application input validation), US9019840B2 (CFM for conflicting MAC address notification), US8606940B2 (DHCP address conflict detection), WO2018222479A1 (conflict rules in a network intent), US5964871A (resolution of resource conflicts), US8433734B2, US6286013B1, US5689701A, US7444317B2 and US6470345B1 (file names and paths in file systems), US6802067B1, US20160094620A1, US20060048012A1, US7606814B2 and US5857190A (logging and tracing), US10318280B2 and US8774029B1 (configuration deployment and patching), US20050262345A1 and US7664948B2 (provider SPI, certificate validation), US8572710B2 (pluggable token provider), US9652214B1 (pluggable extension of software applications), US20150278386A1 (universal XML validator), US7542982B2 (message validation model). Their subjects differ from the ideas above or the snippet showed no matching mechanism; a read is owed if the modules grow toward them. For idea 27's privileged-port declaration with a reason, idea 28's wildcard-aware collision check, idea 29's platform-parameterised lexical path rules, and idea 30's ceiling on log verbosity, the searches found no patent with a matching mechanism (a null from one search each, not a clearance).

### Needs review (to the PM) for row 11

None. Every hit read is outside, as read, with the reason in the table. Hit 32 (US20040019670A1, active to 2029) is the closest; its row says when to read it again. No design change was made.

### Limitations for row 11

Same as above: AI-assisted, one engine, 6 queries with one search each, 2026-10-30; Espacenet, WIPO and the USPTO were not queried and no CPC-class search was run (for example G06F 9/44 for software configuration, G06F 21/60 for protecting data). Status and expiry are Google's labels; continuations and family members were not checked. Not legal advice and not a freedom-to-operate opinion.
