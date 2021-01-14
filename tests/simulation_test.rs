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

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct UserName(String);

const TEST_USERS: &[&str] = &[
    "Alice", "Bob", "Carol", "David", "Erin", "Frank", "Greta", "Holger", "Isabelle", "Jacob",
    "Kate", "Larry", "Margaret", "Noah", "Olivia", "Paul", "Quinn", "Robert", "Susan", "Thomas",
    "Ursula", "Vincent", "Wanda", "Xavier", "Yvonne", "Zachary",
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
            Op::LoginWithCorrectPw(user_id) => {
                if let Some(pass) = registered.get(&user_id) {
                    let auth_header = auth_header(&user_id, &pass);
                    match login(&db, &auth_header) {
                        Ok(true) => {
                            sessions.insert(user_id);
                        }
                        Ok(false) => {
                            return Ok(false);
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
                    Some(existing_pw) => match login(&db, &auth_header) {
                        Ok(_) => return Ok(false),
                        Err(LoginError::InvalidCredentials) => {}
                        Err(e) => {
                            assert_failpoint_err(e)?;
                        }
                    },
                    None => match login(&db, &auth_header) {
                        Ok(true) => return Ok(false),
                        Ok(false) => {}
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
                    Ok(()) => {}
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

        for (registered, pass) in &registered {
            if not_registered.contains(&registered) {
                bail!("{:?} in registered and unregistered at once", registered);
            }
        }
        for session in &sessions {
            if no_session.contains(&session) {
                bail!("{:?} in session and no_session at once");
            }
        }
    }
    Ok(true)
}

#[quickcheck]
fn simulate_login(ops: Vec<Op>) -> anyhow::Result<bool> {
    run_simulator(ops)
}

#[test]
fn regression1() {
    use Op::*;
    let ops = vec![
        LoginWithCorrectPw(UserId("Kate".to_string())),
        AccessSecret(UserId("Jacob".to_string())),
        Register(UserId("Paul".to_string()), Pass("+_".to_string())),
        Fail("db.get_pw".to_string()),
        LoginWithWrongPw(UserId("Zachary".to_string())),
        Register(UserId("Vincent".to_string()), Pass("\u{fd203}".to_string())),
        AccessSecret(UserId("Ursula".to_string())),
        Register(UserId("Xavier".to_string()), Pass("큓\\↑¦⁞".to_string())),
        Logout(UserId("Isabelle".to_string())),
        Fail("db.remove_session".to_string()),
        LoginWithCorrectPw(UserId("Jacob".to_string())),
        Fail("db.get_pw".to_string()),
        LoginWithCorrectPw(UserId("Larry".to_string())),
        LoginWithWrongPw(UserId("Kate".to_string())),
        Fail("db.remove_session".to_string()),
        Register(UserId("Kate".to_string()), Pass("¤-\u{7818d}".to_string())),
        LoginWithCorrectPw(UserId("Ursula".to_string())),
        Register(
            UserId("Robert".to_string()),
            Pass("`※\u{fff7}(]~\"\u{603}®뇆=;嵂;G¥£".to_string()),
        ),
        LoginWithWrongPw(UserId("Greta".to_string())),
        Logout(UserId("Olivia".to_string())),
        Register(
            UserId("Greta".to_string()),
            Pass("D1—\u{10fffd}5".to_string()),
        ),
        AccessSecret(UserId("Ursula".to_string())),
        AccessSecret(UserId("Noah".to_string())),
        Fail("db.remove_session".to_string()),
        Register(UserId("Carol".to_string()), Pass("ue\u{601}".to_string())),
        Fail("db.get_pw".to_string()),
        LoginWithCorrectPw(UserId("Olivia".to_string())),
        AccessSecret(UserId("Paul".to_string())),
        LoginWithWrongPw(UserId("Xavier".to_string())),
        AccessSecret(UserId("Thomas".to_string())),
        Logout(UserId("Jacob".to_string())),
        Logout(UserId("Olivia".to_string())),
        Fail("db.remove_session".to_string()),
        Logout(UserId("Alice".to_string())),
        Logout(UserId("Quinn".to_string())),
        LoginWithWrongPw(UserId("Carol".to_string())),
        Register(
            UserId("Isabelle".to_string()),
            Pass("?\u{90d5b}\u{ad}".to_string()),
        ),
        Fail("db.register".to_string()),
        LoginWithWrongPw(UserId("Yvonne".to_string())),
        LoginWithCorrectPw(UserId("Quinn".to_string())),
        Register(UserId("Greta".to_string()), Pass("T".to_string())),
        LoginWithCorrectPw(UserId("Quinn".to_string())),
        Logout(UserId("Kate".to_string())),
        LoginWithWrongPw(UserId("Frank".to_string())),
        AccessSecret(UserId("Paul".to_string())),
        LoginWithCorrectPw(UserId("Zachary".to_string())),
        Register(
            UserId("Wanda".to_string()),
            Pass("0\u{ac89d}\u{205f}~".to_string()),
        ),
        LoginWithWrongPw(UserId("Susan".to_string())),
        LoginWithCorrectPw(UserId("Robert".to_string())),
    ];
    assert!(run_simulator(ops).unwrap());
}
