//! The platform-independent model: document versions, the diff between
//! them, and the layout the renderer draws. No browser or GL dependency,
//! so everything here is tested natively with `cargo test`.

/// The crate version, surfaced in the page so a build can be identified.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_the_package_version() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
