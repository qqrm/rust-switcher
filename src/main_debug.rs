#![allow(
    clippy::multiple_crate_versions,
    reason = "The Windows stack and language-detection dependencies currently require parallel transitive versions."
)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

include!("main_body.rs");
