#![feature(format_args_capture)]

pub mod api;
pub mod domain;
pub mod in_memory_db;

pub use domain::{
    can_access_secret, db, login, logout, register, EncodedPassword, EnteredPassword, LoginError,
    LogoutError, RegisterError, UserId,
};
