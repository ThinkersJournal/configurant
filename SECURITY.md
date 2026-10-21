# Security policy

Report suspected vulnerabilities privately through GitHub's private vulnerability reporting for this
repository (**Security** tab, then **Report a vulnerability**). Please do not open a public issue.

In scope: a secret value reaching an error message, `Debug`/`Display` output, `explain`, `--help`, a
file or a command line; a secret accepted from a source the schema forbids; an unknown key accepted
and silently ignored; a layer that overrides one of higher precedence.
