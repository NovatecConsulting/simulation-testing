use model_testing::{EnteredPassword, UserId};
use quickcheck::Arbitrary;

#[derive(Clone)]
enum Op {
    Register(UserName),
    Login(UserName),
    Logout(UserName),
    AccessSecret(UserName),
    Fail(String),
}

#[derive(Clone)]
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
            return Op::Fail(g.choose(&fail::list()).unwrap().0.clone());
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
