# Changelog

## Unreleased (PM allocates the number)

- `Config::deserialize::<T>()`: opt-in serde struct binding by dotted key; no codegen. A serde error that
  would quote a secret value is withheld (`ConfigError::Deserialize`, a new variant).

## 0.0.1

- Repository scaffold: CI (jobs build-test, msrv, deny, spdx), `deny.toml`, `CODEOWNERS`, `SECURITY.md`.
- Schema types and strict parser with self-checks; layering (default < file < env < CLI); validation with
  named errors (all reported); `explain`; secrets enforced; the module interface with `ChoiceList`.
- `configurant check|describe|explain`.
