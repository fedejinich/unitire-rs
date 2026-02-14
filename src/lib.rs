#![forbid(unsafe_code)]

pub const CRATE_NAME: &str = "unitrie-rs-core";

pub fn crate_name() -> &'static str {
    CRATE_NAME
}
