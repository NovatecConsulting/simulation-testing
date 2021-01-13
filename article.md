# Trying out simulation testing

The plan is to try out Simulation Testing/Model Testing to see how hard it is to get going and how it compares to less esoteric testing.
Resources:
* https://sled.rs/simulation.html
* https://www.youtube.com/watch?v=4fFDFbi3toc
* https://medium.com/@tylerneely/reliable-systems-series-model-based-property-testing-e89a433b360

## First: Property-Based Testing

Starting with what I think is generally accepted to be a good approach:
A hexagonal architecture (also called "ports and adapter"), using property-based tests where possible, strictly (not stringly) typed APIs.

When starting out writing properties without ever writing a very basic happy-path test, and the property finds a failure, it's very possible to think the property found a bug about an obscure edge-case.
Make sure that the happy path even works though.
In my case, the property tests didn't find any bugs that basic happy-path unit tests wouldn't have found (I forgot to hash the passwords before storing them, but then passed the plaintext passwords to a hash decode function, which failed).

TODO: Introduce simulation/model tests!

Nice tidbits: I used different typed for entered passwords (the data that is received from the user and passed to hashing functions to be stored or compared with an already stored passwords) and encoded passwords (which are stored).
The entered passwords should never be seen by anyone, so no printing them ever! The encoded ones are in theory safe to see, but it's better not to.
When writing tests, I do want to be able to see the contents of the mock database though, which would require printing the passwords.
Conditional compilation to the rescue!
A `#[cfg_attr(test, derive(Debug))]` attribute on my password types and my Db type allow me to print them in tests, but not in production code.


