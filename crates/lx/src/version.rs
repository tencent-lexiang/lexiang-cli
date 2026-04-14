pub fn current_version() -> &'static str {
    resolve_version(option_env!("LX_VERSION"), env!("CARGO_PKG_VERSION"))
}

fn resolve_version(injected: Option<&'static str>, cargo: &'static str) -> &'static str {
    injected.unwrap_or(cargo)
}

#[cfg(test)]
mod tests {
    use super::resolve_version;

    #[test]
    fn prefers_injected_version_when_present() {
        assert_eq!(resolve_version(Some("1.2.3"), "0.1.0"), "1.2.3");
    }

    #[test]
    fn falls_back_to_cargo_version_when_not_injected() {
        assert_eq!(resolve_version(None, "0.1.0"), "0.1.0");
    }
}
