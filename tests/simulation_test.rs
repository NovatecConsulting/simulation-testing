use anyhow::anyhow;
use fail::fail_point;
use model_testing::{
    db::{Db, DbError, DbResult},
    domain::EncodedPassword,
    in_memory_db::{self, init_db},
    EnteredPassword, UserId,
};
use quickcheck::Arbitrary;
use quickcheck_macros::quickcheck;

#[derive(Clone, Debug)]
enum Op {
    Register(UserName),
    Login(UserName),
    Logout(UserName),
    AccessSecret(UserName),
    Fail(String),
}

#[derive(Clone, Debug)]
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

impl Arbitrary for Op {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        if u8::arbitrary(g) < 20 {
            let fail_points = fail::list();
            eprintln!("Fail points: {:?}", fail_points);
            if !fail_points.is_empty() {
                return Op::Fail(g.choose(&fail::list()).unwrap().0.clone());
            }
        }

        let user_id = UserName::arbitrary(g);
        g.choose(&[
            Op::Register(user_id.clone()),
            Op::Login(user_id.clone()),
            Op::Logout(user_id.clone()),
            Op::AccessSecret(user_id),
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

#[quickcheck]
fn simulate_login(ops: Vec<Op>) -> bool {
    let db = FailDb::new(in_memory_db::init_db());
    false
}
