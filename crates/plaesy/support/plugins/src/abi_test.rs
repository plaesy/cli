use super::*;

#[test]
fn parses_major_and_minor() {
    assert_eq!("1.0".parse::<Abi>().unwrap(), Abi::new(1, 0));
    assert_eq!("12.345".parse::<Abi>().unwrap(), Abi::new(12, 345));
}

#[test]
fn rejects_malformed_abi() {
    for raw in ["", "1", "1.0.0", "a.0", "+1.0", "1.-1", "1.", ".1", "1..0"] {
        assert_eq!(
            raw.parse::<Abi>(),
            Err(PluginError::InvalidAbi(raw.to_owned())),
            "{raw:?}"
        );
    }
}

#[test]
fn displays_as_major_dot_minor() {
    assert_eq!(Abi::new(2, 7).to_string(), "2.7");
    assert_eq!(HOST_ABI.to_string(), "1.0");
}

#[test]
fn accepts_same_major_and_not_newer_minor() {
    assert!(check_compatible(Abi::new(1, 0), HOST_ABI).is_ok());
    assert!(check_compatible(Abi::new(1, 0), Abi::new(1, 3)).is_ok());
}

#[test]
fn rejects_other_major_or_newer_minor() {
    for plugin in [Abi::new(2, 0), Abi::new(0, 9), Abi::new(1, 1)] {
        assert_eq!(
            check_compatible(plugin, HOST_ABI),
            Err(PluginError::UnsupportedAbi {
                plugin: plugin.to_string(),
                host: HOST_ABI.to_string(),
            })
        );
    }
}

#[test]
fn parse_number_only_accepts_plain_digits() {
    assert_eq!(parse_number("007"), Some(7));
    assert_eq!(parse_number("+7"), None);
    assert_eq!(parse_number("-7"), None);
    assert_eq!(parse_number(""), None);
    assert_eq!(parse_number("99999999999"), None);
}
