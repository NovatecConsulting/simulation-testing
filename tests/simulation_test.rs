#![feature(format_args_capture)]

use std::{
    collections::{HashMap, HashSet},
    error,
};

use anyhow::{anyhow, bail};
use error::Error;
use fail::fail_point;
use model_testing::{
    can_access_secret,
    db::{Db, DbResult},
    in_memory_db, login, logout, register, EncodedPassword, EnteredPassword, LoginError, UserId,
};
use quickcheck::Arbitrary;
use quickcheck_macros::quickcheck;

#[derive(Clone, Debug)]
enum Op {
    Register(UserId, Pass),
    LoginWithCorrectPw(UserId),
    LoginWithWrongPw(UserId),
    Logout(UserId),
    AccessSecret(UserId),
    Fail(String),
}

use Op::*;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct UserName(String);

const TEST_USERS: &[&str] = &[
    "Alice", "Bob", "Carol", "David", "Erin", "Frank"
    // , "Greta", "Holger", "Isabelle", "Jacob",
    // "Kate", "Larry", "Margaret", "Noah", "Olivia", "Paul", "Quinn", "Robert", "Susan", "Thomas",
    // "Ursula", "Vincent", "Wanda", "Xavier", "Yvonne", "Zachary",
];

impl Arbitrary for UserName {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        UserName(g.choose(TEST_USERS).unwrap().to_string())
    }
}

impl UserName {
    fn id(&self) -> UserId {
        UserId(self.0.clone())
    }
}

#[derive(Debug, Clone)]
struct Pass(String);

impl Arbitrary for Pass {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        let mut s = String::arbitrary(g);
        while s.is_empty() || s.chars().any(|c| c.is_control()) {
            s = String::arbitrary(g);
        }
        Self(s)
    }
}

impl Pass {
    fn entered_password(&self) -> EnteredPassword {
        EnteredPassword::new(self.0.clone())
    }
}

impl Arbitrary for Op {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        if u8::arbitrary(g) < 20 {
            let fail_points = vec![
                "db.register",
                "db.add_session",
                "db.remove_session",
                "db.get_pw",
                "db.has_session",
            ];
            if !fail_points.is_empty() {
                return Op::Fail(g.choose(&fail_points).unwrap().to_string());
            }
        }

        let user_id = UserName::arbitrary(g);
        let pass = Pass::arbitrary(g);
        g.choose(&[
            Op::Register(user_id.id(), pass),
            Op::LoginWithCorrectPw(user_id.id()),
            Op::LoginWithWrongPw(user_id.id()),
            Op::Logout(user_id.id()),
            Op::AccessSecret(user_id.id()),
        ])
        .unwrap()
        .clone()
    }
}

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
        fail_point!("db.register", |_| Err(
            anyhow!("db.register failpoint").into()
        ));
        self.inner.register(user_id, password)
    }

    fn add_session(&self, user_id: UserId) -> DbResult {
        fail_point!("db.add_session", |_| Err(anyhow!(
            "db.add_session failpoint"
        )
        .into()));
        self.inner.add_session(user_id)
    }

    fn remove_session(&self, user_id: &UserId) -> DbResult {
        fail_point!("db.remove_session", |_| Err(anyhow!(
            "db.remove_session failpoint"
        )
        .into()));
        self.inner.remove_session(user_id)
    }

    fn get_pw(&self, user_id: &UserId) -> DbResult<Option<EncodedPassword>> {
        fail_point!("db.get_pw", |_| Err(anyhow!("db.get_pw failpoint").into()));
        self.inner.get_pw(user_id)
    }

    fn has_session(&self, user_id: &UserId) -> DbResult<bool> {
        fail_point!("db.has_session", |_| Err(anyhow!(
            "db.has_session failpoint"
        )
        .into()));
        self.inner.has_session(user_id)
    }
}
fn auth_header(user: &UserId, pass: &Pass) -> String {
    let encoded = base64::encode(format!("{}:{}", user.0, pass.0));
    format!("Basic {encoded}")
}

fn assert_failpoint_err(e: impl Error + Send + Sync + 'static) -> anyhow::Result<()> {
    if e.to_string().contains("failpoint") {
        Ok(())
    } else {
        Err(e.into())
    }
}

fn run_simulator(ops: Vec<Op>) -> anyhow::Result<bool> {
    // eprintln!("simulating ops {:?}", ops);
    let db = FailDb::new(in_memory_db::init_db());
    let mut not_registered = HashSet::new();
    let mut registered = HashMap::new();
    let mut sessions = HashSet::new();
    let mut no_session = HashSet::new();
    for op in ops {
        // eprintln!("Handling Op {:?}", op);
        match op {
            Op::Register(user_id, pass) => {
                if !registered.contains_key(&user_id) {
                    match register(&db, user_id.clone(), pass.entered_password()) {
                        Ok(()) => {
                            not_registered.remove(&user_id);
                            registered.insert(user_id, pass);
                        }
                        Err(e) => {
                            assert_failpoint_err(e)?;
                            not_registered.insert(user_id);
                        }
                    }
                }
            }
            Op::LoginWithCorrectPw(user_id) => {
                if let Some(pass) = registered.get(&user_id) {
                    let auth_header = auth_header(&user_id, &pass);
                    match login(&db, &auth_header) {
                        Ok(()) => {
                            sessions.insert(user_id);
                        }
                        Err(e) => {
                            assert_failpoint_err(e)?;
                            no_session.insert(user_id);
                        }
                    }
                }
            }
            Op::LoginWithWrongPw(user_id) => {
                let wrong_pw = Pass("hunter2".to_string());
                let auth_header = auth_header(&user_id, &wrong_pw);
                match registered.get(&user_id) {
                    Some(_existing_pw) => match login(&db, &auth_header) {
                        Ok(_) => return Ok(false),
                        Err(LoginError::InvalidCredentials) => {}
                        Err(e) => {
                            assert_failpoint_err(e)?;
                        }
                    },
                    None => match login(&db, &auth_header) {
                        Ok(()) => return Ok(false),
                        Err(LoginError::NotRegistered) => {}
                        Err(e) => {
                            assert_failpoint_err(e)?;
                        }
                    },
                };
            }
            Op::Logout(user_id) => {
                let pass = registered
                    .get(&user_id)
                    .cloned()
                    .unwrap_or(Pass("hunter2".to_string()));
                let auth_header = auth_header(&user_id, &pass);
                match logout(&db, &auth_header) {
                    Ok(()) => {
                        sessions.remove(&user_id);
                    }
                    Err(e) => {
                        assert_failpoint_err(e)?;
                    }
                }
            }
            Op::AccessSecret(user_id) => match can_access_secret(&db, &user_id) {
                Ok(b) => {
                    if sessions.contains(&user_id) != b {
                        return Ok(false);
                    }
                }
                Err(e) => {
                    assert_failpoint_err(e)?;
                }
            },
            Op::Fail(fail_point_name) => {
                fail::cfg(fail_point_name, "return").unwrap();
            }
        }

        for (user_id, pass) in &registered {
            if not_registered.contains(&user_id) {
                bail!("{:?} in registered and unregistered at once", user_id);
            }
            let auth_header = auth_header(user_id, pass);
            if sessions.contains(user_id) {
                match logout(&db, &auth_header) {
                    Ok(()) => {
                        if let Err(e) = login(&db, &auth_header) {
                            assert_failpoint_err(e)?;
                            sessions.remove(user_id);
                        }
                    }
                    Err(e) => {
                        assert_failpoint_err(e)?;
                    }
                }
            } else {
                match can_access_secret(&db, user_id) {
                    Ok(true) => {
                        bail!("{:?} has no session but can access secret", user_id);
                    }
                    Ok(false) => {}
                    Err(e) => {
                        assert_failpoint_err(e)?;
                    }
                }
                match login(&db, &auth_header) {
                    Ok(()) => {
                        if let Err(e) = logout(&db, &auth_header) {
                            assert_failpoint_err(e)?;
                            sessions.insert(user_id.clone());
                            no_session.remove(user_id);
                        }
                    }
                    Err(e) => {
                        assert_failpoint_err(e)?;
                    }
                }
            }
        }
        for session in &sessions {
            if no_session.contains(&session) {
                bail!("{:?} in session and no_session at once", session);
            }
            if !registered.contains_key(session) {
                bail!("{:?} in session but not registered", session);
            }
            match can_access_secret(&db, session) {
                Ok(true) => {}
                Ok(false) => {
                    bail!("{:?} in session but can't access secret", session);
                }
                Err(e) => {
                    assert_failpoint_err(e)?;
                }
            }
        }
    }
    Ok(true)
}

#[quickcheck]
fn simulate_login(ops: Vec<Op>) -> anyhow::Result<bool> {
    eprintln!("Called with {} Ops {:?}", ops.len(), ops);
    dbg!(run_simulator(ops))
}

#[test]
fn regression1() {
    let ops = vec![
        Register(
            UserId("Greta".to_string()),
            Pass("D1—\u{10fffd}5".to_string()),
        ),
        Fail("db.register".to_string()),
        Register(UserId("Greta".to_string()), Pass("T".to_string())),
    ];
    assert!(run_simulator(ops).unwrap());
}

#[test]
fn custom_bug() {
    let ops = vec![
        Register(UserId("Alice".to_string()), Pass("A".to_string())),
        Register(UserId("Bob".to_string()), Pass("B".to_string())),
        Register(UserId("Carol".to_string()), Pass("C".to_string())),
        Register(UserId("David".to_string()), Pass("D".to_string())),
        Register(UserId("Erin".to_string()), Pass("E".to_string())),
        LoginWithCorrectPw(UserId("Alice".to_string())),
        LoginWithCorrectPw(UserId("Bob".to_string())),
        LoginWithCorrectPw(UserId("Carol".to_string())),
        LoginWithCorrectPw(UserId("David".to_string())),
        LoginWithCorrectPw(UserId("Erin".to_string())),
    ];
    assert!(run_simulator(ops).unwrap());
}
