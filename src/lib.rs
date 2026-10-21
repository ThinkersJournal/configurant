// SPDX-License-Identifier: MIT OR Apache-2.0
//! ConfigurAnt: schema-first configuration. A project declares its values in `configurant.toml`;
//! this library parses the schema, layers values (default < file < environment < command line),
//! validates them with named errors, and explains where each value came from. It generates no
//! code, and a value marked `secret` is never printed. See `README.md` and `docs/DESIGN.md`.

pub mod config;
pub mod error;
pub mod module;
pub mod schema;
pub mod value;

pub use config::{Config, DEFAULT_ORDER, ExplainLine, Explanation, FromValue, Layer, Loader};
pub use error::{ConfigError, Errors, Origin};
pub use module::{
    Action, ActionKind, Choice, ChoiceList, INTERFACE_VERSION, Module, ModuleError, Modules, Needs,
    Plan,
};
pub use schema::{Project, Schema, SchemaError, ValueDef};
pub use value::{Value, ValueType};
