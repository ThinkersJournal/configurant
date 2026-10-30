// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ready-made [`crate::Module`]s, each behind its own Cargo feature so a project that wants none carries
//! none. Design and rules: `docs/MODULES.md`. A project registers the ones it uses; a schema value that
//! names an unregistered module is still refused, naming the module.

#[cfg(feature = "logging")]
pub mod logging;
#[cfg(feature = "paths")]
pub mod paths;
#[cfg(feature = "ports")]
pub mod ports;
