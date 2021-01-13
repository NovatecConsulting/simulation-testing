#![feature(format_args_capture)]

pub mod api;
pub mod domain;
pub mod in_memory_db;

pub use domain::{
    login, logout, register, EnteredPassword, LoginError, LogoutError, RegisterError, UserId,
};
