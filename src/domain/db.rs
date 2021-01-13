use super::{EncodedPassword, UserId};

pub type DbResult<T = ()> = Result<T, DbError>;

#[derive(thiserror::Error, Debug)]
#[error("Db Error: {inner}")]
pub struct DbError {
    #[from]
    inner: anyhow::Error,
}
pub trait Db {
    fn register(&self, user_id: UserId, password: EncodedPassword) -> DbResult;
    fn add_session(&self, user_id: UserId) -> DbResult;
    fn remove_session(&self, user_id: &UserId) -> DbResult;
    fn get_pw(&self, user_id: &UserId) -> DbResult<Option<EncodedPassword>>;
    fn has_session(&self, user_id: &UserId) -> DbResult<bool>;
}
