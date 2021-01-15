# Trying out simulation testing

The plan is to try out Simulation Testing/Model Testing to see how hard it is to get going and how it compares to less esoteric testing.
Resources:
* https://sled.rs/simulation.html
* https://www.youtube.com/watch?v=4fFDFbi3toc
* https://medium.com/@tylerneely/reliable-systems-series-model-based-property-testing-e89a433b360

## First: Property-Based Testing

Starting with what I think is generally accepted to be a good approach:
A hexagonal architecture (also called "ports and adapter"), using property-based tests where possible, strictly (not stringly) typed APIs.

Tests look like this:

```rust
    #[quickcheck]
    fn cant_login_without_registering(user: UserId, pass: EnteredPassword) -> bool {
        let header = auth_header(&user, &pass);
        let db = in_memory_db::init_db();
        !login(&db, &header).unwrap()
    }

    #[quickcheck]
    fn can_access_secrets_after_logging_in(user: UserId, pass: EnteredPassword) -> bool {
        let header = auth_header(&user, &pass);
        let db = in_memory_db::init_db();
        register(&db, user.clone(), pass.clone()).unwrap();
        login(&db, &header).unwrap();
        can_access_secret(&db, &user).unwrap()
    }
```

When starting out writing properties without ever writing a very basic happy-path test, and the property finds a failure, it's very possible to think the property found a bug about an obscure edge-case.
Make sure that the happy path even works though.

The first property failures looked quite obscure (not sure why shrinking didn't help there), but in the end I just forgot to hash my passwords before storing them, so I passed the plaintext passwords to the hash_decode function which didn't work.

Apart from that, the property tests found one bug that basic happy-path unit tests might not have found:
Usernames can't have colons in them if Basic Auth is used.

## Second: Model Testing

Here, random inputs to all operations of the system are generated and a simplified model is used to check invariants.
Then, a property-based testing tool is used to generate random input to the system, and if any combination of inputs leads to a failure, shrinking can help find the minimal input that causes the failure.

This looks like:

```rust
enum Op {
    Register(UserId, Pass),
    LoginWithCorrectPw(UserId),
    LoginWithWrongPw(UserId),
    Logout(UserId),
    AccessSecret(UserId),
    Fail(String),
}
impl Arbitrary for Op { ... }
```

Which invariants to check and what kind of model to use though?

I did something which might be a bit too close to a parallel implementation (which is hard to avoid with an implementation that is mostly based on two hashmaps...).

So far I only found one trivial bug in my simulator code (which again failed with 60 operations, I need to check the shrinking...).
Maybe I need to go further and implement full-blown simulation testing...

I also tried to test the bug-finding powers by introducing a semi-obscure bug - overwriting the password of an existing user with the password of a new user.
I started off only overwriting existing passwords after the 16th user, which the simulation didn't hit once after trying out 10 or 20 times.
I guess this is to be expected though.
After changing the bug to nearly always modifying another user's password, the simulation finds it more often than not (although not always).
The shrinking results often contain garbage, maybe it would help to use a `Vec`-wrapper that uses the same `arbitrary` definition but exhaustively searches sublists.
Might be worth a try.

## Third: Simulation Testing

Haven't started yet, not sure if I will.
This will introduce pseudo-concurrency under strict deterministic control by the test runner, possibly implemented via a Futures reactor...
Does something like this already exist?

## Nice things about Rust

I used different typed for entered passwords (the data that is received from the user and passed to hashing functions to be stored or compared with an already stored passwords) and encoded passwords (which are stored).
The entered passwords should never be seen by anyone, so no printing them ever! The encoded ones are in theory safe to see, but it's better not to.
When writing tests, I do want to be able to see the contents of the mock database though, which would require printing the passwords.
Conditional compilation to the rescue!
A `#[cfg_attr(test, derive(Debug))]` attribute on my password types and my Db type allow me to print them in tests, but not in production code.


