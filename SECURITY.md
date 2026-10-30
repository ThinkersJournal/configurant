# Security policy

Report suspected vulnerabilities privately through GitHub's private vulnerability reporting for this
repository (**Security** tab, then **Report a vulnerability**). Please do not open a public issue.

In scope: a secret value reaching an error message, `Debug`/`Display` output, `explain`, `--help`, a
file or a command line; a secret accepted from a source the schema forbids; an unknown key accepted
and silently ignored; a layer that overrides one of higher precedence.

## Supported versions

The latest release line receives security fixes. An older line receives a backport when the backport is small. When it is not, we do not attempt it: the advisory says that line is not fixed and recommends upgrading. No support period is promised.
