//! One list of credential prefixes: the session executor withholds a line of a harness child's
//! stderr for exactly the prefixes an agent spec is refused for (`SecretLiteral`), so a prefix
//! added to one is added to the other.

#[test]
fn the_executor_withholds_stderr_for_the_prefixes_an_agent_spec_is_refused_for() {
    assert_eq!(
        loom_sdk::session::TOKEN_PREFIXES,
        b10x_loom_agent::TOKEN_PREFIXES,
        "loom_sdk::session::TOKEN_PREFIXES and b10x_loom_agent::TOKEN_PREFIXES differ"
    );
}
