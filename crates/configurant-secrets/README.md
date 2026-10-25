# configurant-secrets

Secret sources for [ConfigurAnt](https://github.com/ThinkersJournal/configurant). A schema value marked
`secret = true` may name a source, `source = "<source>:<reference>"`; this crate fetches the secret by that
reference at load time and hands it to `Loader::resolved`, where it is checked and redacted like any secret.
The environment still overrides it, and a file, the command line and a default stay refused. Design:
`docs/SECRET-SOURCES.md`.

    let mut sources = Sources::new();
    sources.register(Box::new(MySource))?;
    let resolved = resolve(&schema, &sources, &env, Duration::from_secs(5))?;
    let config = resolved.into_loader(Loader::new(&schema).env(env)).build()?;

A source returns only a `SecretValue` or a failure kind (`NotFound`, `Denied`, `Unreachable`, `Invalid`); it cannot put text
in an error, so no error carries what a source received. A source that does not answer in time is `TimedOut`.
One source is built in, `FileSource` (`source = "file:<name>"`): it reads `<name>` from the directory named by
`CONFIGURANT_SECRETS_DIR`, a bare file name only, at most 64 KiB of UTF-8, a symlink only if it stays inside the
directory, and on Unix only a file with no group or other permission bits. Windows has no permission check; see
`docs/SECRET-SOURCES.md` section 4a. The vault wrapper already delivers an environment variable, so there is no `vault`
source, and a SecretAnt adapter will be its own crate.

Limit: any process running as the same user can read what this process can. `SecretValue` is wiped on drop, but
the loader's copy inside the `Config` is not.

Licensed under MIT OR Apache-2.0.
