//! Stellae Craft Launcher — core engine.
//!
//! This crate is UI-agnostic: it implements everything the launcher needs to
//! authenticate, resolve version metadata, download game files and spawn the
//! JVM. The `stellae-craft-launcher` binary renders it with GPUI Kit.

pub mod auth;
pub mod config;
pub mod download;
pub mod error;
pub mod install;
pub mod instance;
pub mod java;
pub mod launch;
pub mod loader;
pub mod meta;
pub mod net;
pub mod paths;
pub mod skin;

pub use error::{Error, Result};
pub use instance::{Instance, InstanceStore};
pub use paths::LauncherPaths;

/// Launcher identity, injected into the game arguments.
pub const LAUNCHER_NAME: &str = "StellaeCraft";
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
