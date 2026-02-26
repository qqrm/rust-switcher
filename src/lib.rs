#![feature(stmt_expr_attributes)]

#[cfg(windows)]
pub mod app;

pub mod config;

#[cfg(windows)]
pub mod conversion;

pub mod domain;

#[cfg(windows)]
pub mod helpers;

pub mod input;
pub mod input_journal;

#[cfg(windows)]
pub mod platform;

#[cfg(windows)]
pub mod utils;

#[cfg(test)]
mod core_tests;

#[cfg(all(test, windows))]
mod tests;
