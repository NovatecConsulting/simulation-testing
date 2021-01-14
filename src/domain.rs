use std::string::FromUtf8Error;

use uuid::Uuid;

use self::db::{Db, DbError, DbResult};

pub mod db;

pub fn can_access_secret(db: &impl Db, user_id: &UserId) -> DbResult<bool> {
    db.has_session(&user_id)
}

#[derive(thiserror::Error, Debug)]
pub enum LoginError {
    #[error("Invalid Credentials")]
    InvalidCredentials,
    #[error("Failed to process password")]
    HashError(#[from] argon2::Error),
    #[error("{0}")]
    ParseAuthError(#[from] ParseAuthError),
    #[error("{0}")]
    DbError(#[from] DbError),
}

pub fn login(db: &impl Db, auth_header: &str) -> Result<bool, LoginError> {
    let (user_id, pw) = parse_auth(auth_header)?;

    let encoded = match db.get_pw(&user_id)? {
        Some(it) => it,
        None => return Ok(false),
    };
    if encoded.verify(&pw)? {
        db.add_session(user_id)?;
        Ok(true)
    } else {
        Err(LoginError::InvalidCredentials)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LogoutError {
    #[error("{0}")]
    ParseAuthError(#[from] ParseAuthError),
    #[error("{0}")]
    DbError(#[from] DbError),
}

pub fn logout(db: &impl Db, auth_header: &str) -> Result<(), LogoutError> {
    let (user_id, _) = parse_auth(auth_header)?;

    db.remove_session(&user_id)?;

    Ok(())
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum ParseAuthError {
    #[error("Malformed Header")]
    MalformedHeader,
    #[error("UTF8 conversion failed")]
    Utf8Error(#[from] FromUtf8Error),
}

fn parse_auth(auth_header: &str) -> Result<(UserId, EnteredPassword), ParseAuthError> {
    const BASIC: &str = "Basic ";

    if !auth_header.starts_with(BASIC) {
        return Err(ParseAuthError::MalformedHeader);
    }
    let auth = &auth_header[BASIC.len()..];
    let auth = base64::decode(auth).map_err(|_| ParseAuthError::MalformedHeader)?;
    let auth = String::from_utf8(auth)?;
    let parts = auth.splitn(2, ':').collect::<Vec<_>>();
    match parts.as_slice() {
        &[user, pass] => (Ok((UserId(user.to_string()), EnteredPassword(pass.to_string())))),
        _ => Err(ParseAuthError::MalformedHeader),
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct UserId(pub String);
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct EncodedPassword(String);

impl EncodedPassword {
    fn verify(&self, entered_password: &EnteredPassword) -> Result<bool, argon2::Error> {
        argon2::verify_encoded(self.0.as_str(), entered_password.0.as_bytes())
    }
}

#[derive(PartialEq, Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct EnteredPassword(String);

impl EnteredPassword {
    pub fn new(s: String) -> Self {
        Self(s)
    }
    pub fn encode(self) -> Result<EncodedPassword, argon2::Error> {
        let salt = Uuid::new_v4();
        let encoded = argon2::hash_encoded(
            self.0.as_bytes(),
            salt.as_bytes(),
            &argon2::Config::default(),
        )?;
        Ok(EncodedPassword(encoded))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RegisterError {
    #[error("Failed to process password")]
    HashError(#[from] argon2::Error),
    #[error("{0}")]
    DbError(#[from] DbError),
}

pub fn register(db: &impl Db, user_id: UserId, pass: EnteredPassword) -> Result<(), RegisterError> {
    Ok(db.register(user_id, pass.encode()?)?)
}

#[cfg(test)]
mod property_tests {
    use crate::in_memory_db;

    use super::*;
    use quickcheck::Arbitrary;
    use quickcheck_macros::quickcheck;

    impl Arbitrary for UserId {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut s = String::arbitrary(g);
            while s.is_ascii() || s.contains(':') || s.chars().any(|c| c.is_control()) {
                s = String::arbitrary(g);
            }
            UserId(s)
        }
    }

    impl Arbitrary for EnteredPassword {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut s = String::arbitrary(g);
            while s.is_empty() || s.chars().any(|c| c.is_control()) {
                s = String::arbitrary(g);
            }
            EnteredPassword(s)
        }
    }

    fn auth_header(user: &UserId, pass: &EnteredPassword) -> String {
        let encoded = base64::encode(format!("{}:{}", user.0, pass.0));
        format!("Basic {encoded}")
    }

    #[quickcheck]
    fn parse_basic_auth_roundtrip(user: UserId, pass: EnteredPassword) -> bool {
        let encoded = base64::encode(format!("{}:{}", user.0, pass.0));
        let header = format!("Basic {encoded}");
        parse_auth(&header) == Ok((user, pass))
    }

    #[quickcheck]
    fn cant_access_secret_without_logging_in(user: UserId) -> bool {
        let db = in_memory_db::init_db();
        !can_access_secret(&db, &user).unwrap()
    }

    #[quickcheck]
    fn cant_login_without_registering(user: UserId, pass: EnteredPassword) -> bool {
        let header = auth_header(&user, &pass);
        let db = in_memory_db::init_db();
        !login(&db, &header).unwrap()
    }

    #[quickcheck]
    fn can_login_after_registering(user: UserId, pass: EnteredPassword) -> bool {
        let header = auth_header(&user, &pass);
        let db = in_memory_db::init_db();
        register(&db, user.clone(), pass.clone()).unwrap();
        login(&db, &header).unwrap()
    }

    #[quickcheck]
    fn can_access_secrets_after_logging_in(user: UserId, pass: EnteredPassword) -> bool {
        let header = auth_header(&user, &pass);
        let db = in_memory_db::init_db();
        register(&db, user.clone(), pass.clone()).unwrap();
        login(&db, &header).unwrap();
        can_access_secret(&db, &user).unwrap()
    }

    #[quickcheck]
    fn cant_access_secrets_after_logging_in_and_out(user: UserId, pass: EnteredPassword) -> bool {
        let header = auth_header(&user, &pass);
        let db = in_memory_db::init_db();
        login(&db, &header).unwrap();
        logout(&db, &header).unwrap();
        !can_access_secret(&db, &user).unwrap()
    }
}
