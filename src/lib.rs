#![feature(stmt_expr_attributes)]

#[cfg(windows)]
pub mod app;

pub mod config;

#[cfg(windows)]
mod conversion;

mod domain;

#[cfg(windows)]
mod helpers;

mod input;
mod input_journal;

#[cfg(windows)]
mod platform;

#[cfg(windows)]
mod utils;

#[cfg(test)]
mod core_tests;
