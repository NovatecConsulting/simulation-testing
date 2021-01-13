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

#[cfg(test)]
mod failing_db {
    use fail::fail_point;

    use super::*;

    struct FailDb {
        inner: Box<dyn Db>,
    }

    impl FailDb {
        fn new(inner: impl Db + 'static) -> Self {
            Self {
                inner: Box::new(inner),
            }
        }
    }

    impl Db for FailDb {
        fn register(&self, user_id: UserId, password: EncodedPassword) -> DbResult {
            fail_point!("db.register", |_| Err(DbError {
                inner: anyhow!("db.register failpoint")
            }));
            self.inner.register(user_id, password)
        }

        fn add_session(&self, user_id: UserId) -> DbResult {
            fail_point!("db.add_session", |_| Err(DbError {
                inner: anyhow!("db.add_session failpoint")
            }));
            self.inner.add_session(user_id)
        }

        fn remove_session(&self, user_id: &UserId) -> DbResult {
            fail_point!("db.remove_session", |_| Err(DbError {
                inner: anyhow!("db.remove_session failpoint")
            }));
            self.inner.remove_session(user_id)
        }

        fn get_pw(&self, user_id: &UserId) -> DbResult<Option<EncodedPassword>> {
            fail_point!("db.get_pw", |_| Err(DbError {
                inner: anyhow!("db.get_pw failpoint")
            }));
            self.inner.get_pw(user_id)
        }

        fn has_session(&self, user_id: &UserId) -> DbResult<bool> {
            fail_point!("db.has_session", |_| Err(DbError {
                inner: anyhow!("db.has_session failpoint")
            }));
            self.inner.has_session(user_id)
        }
    }
}
