//! End-to-end CLI tests: run the built `nose` binary against a temp project and
//! check the user-visible behavior (discovery, `query` report, `--exclude`).

mod fixtures;
mod process;
mod query;

pub(crate) use fixtures::*;
pub(crate) use process::*;
pub(crate) use query::*;
pub(crate) use std::fs;
pub(crate) use std::path::{Path, PathBuf};
pub(crate) use std::process::{Command, Stdio};
