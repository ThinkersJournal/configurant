// SPDX-License-Identifier: MIT OR Apache-2.0
//! ConfigurAnt: schema-first configuration. A project declares its values in `configurant.toml`;
//! this library parses the schema, layers values (default < file < environment < command line),
//! validates them with named errors, and explains where each value came from. It generates no
//! code, and a value marked `secret` is never printed. See `README.md` and `docs/DESIGN.md`.

pub mod config;
pub mod edit;
pub mod error;
pub mod keycheck;
pub mod migrate;
pub mod module;
#[cfg(feature = "persistant")]
pub mod persistant;
pub mod schema;
pub mod schema_edit;
pub mod value;
pub mod view;

pub use config::{Config, DEFAULT_ORDER, ExplainLine, Explanation, FromValue, Layer, Loader};
pub use error::{ConfigError, Errors, Origin};
pub use migrate::{MigrateError, Migrated};
pub use module::{
    Action, ActionKind, Choice, ChoiceList, INTERFACE_VERSION, Module, ModuleError, Modules, Needs,
    Plan,
};
pub use schema::{Bound, MapDef, Migration, Op, Project, Schema, SchemaError, TableDef, ValueDef};
pub use value::{Row, Value, ValueType};
