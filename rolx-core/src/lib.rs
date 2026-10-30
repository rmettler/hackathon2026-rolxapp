//! Frontend-independent logic shared by all RolX client apps (CLI, GUI, ...).

pub mod activities;
pub mod client;
pub mod config;
pub mod dayedit;
pub mod dayplan;
pub mod error;
pub mod idle;
pub mod logging;
pub mod login;
pub mod oauth2;
pub mod records;
pub mod rolx;
pub mod session;
pub mod stamp;
pub mod storage;
pub mod sync;
pub mod undo;
pub mod week;

#[cfg(test)]
mod test_support;
