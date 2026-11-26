use super::*;

fn fs(path: &str) -> Capability {
    Capability::FsRead {
        path: path.to_owned(),
    }
}

fn net(host: &str) -> Capability {
    Capability::NetHost {
        host: host.to_owned(),
    }
}

#[test]
fn normalizes_separators_and_trailing_slash() {
    assert_eq!(normalize_path(r"C:\Data\Notes\").unwrap(), "C:/Data/Notes");
    assert_eq!(normalize_path("/var/data/").unwrap(), "/var/data");
    assert_eq!(normalize_path("rel/dir").unwrap(), "rel/dir");
    assert_eq!(normalize_path("C:").unwrap(), "C:/");
    assert_eq!(normalize_path("C:/x").unwrap(), "C:/x");
    assert_eq!(normalize_path("/").unwrap(), "/");
    assert_eq!(normalize_path("///").unwrap(), "/");
}

#[test]
fn rejects_unsafe_paths() {
    for raw in [
        "", "a/../b", "./a", "a/./b", "a//b", "a\nb", "C:x:y", "/C:/x", "a:b",
    ] {
        assert!(
            matches!(normalize_path(raw), Err(PluginError::InvalidCapability(_))),
            "{raw:?}"
        );
    }
}

#[test]
fn path_covers_by_whole_component() {
    assert!(path_covers("/data", "/data"));
    assert!(path_covers("/data", "/data/notes"));
    assert!(!path_covers("/data", "/database"));
    assert!(!path_covers("/data/notes", "/data"));
    assert!(path_covers("/", "/anything"));
    assert!(path_covers("C:/", "C:/x"));
}

#[test]
fn normalizes_host_and_rejects_wildcards_and_ports() {
    assert_eq!(
        normalize_host("API.Example.COM").unwrap(),
        "api.example.com"
    );
    assert_eq!(normalize_host("127.0.0.1").unwrap(), "127.0.0.1");
    for raw in [
        "*.example.com",
        "example.com:443",
        "a..b",
        "-a.com",
        "a-.com",
        "exa mple.com",
        "",
        "bücher.de",
    ] {
        assert!(
            matches!(normalize_host(raw), Err(PluginError::InvalidCapability(_))),
            "{raw:?}"
        );
    }
}

#[test]
fn grant_is_intersection_of_request_and_approval() {
    let requested = vec![fs("/data/notes"), net("api.example.com"), Capability::Clock];
    let approved = vec![fs("/data"), Capability::Clock];
    let result = grant(&requested, &approved);
    assert_eq!(result.granted, vec![fs("/data/notes"), Capability::Clock]);
    assert_eq!(result.denied, vec![net("api.example.com")]);
}

#[test]
fn approval_never_widens_request() {
    let result = grant(&[fs("/data/notes")], &[fs("/")]);
    assert_eq!(result.granted, vec![fs("/data/notes")]);
    assert!(result.denied.is_empty());
}

#[test]
fn kinds_do_not_cover_each_other() {
    let requested = vec![Capability::FsWrite {
        path: "/data".to_owned(),
    }];
    let result = grant(&requested, &[fs("/data")]);
    assert!(result.granted.is_empty());
    assert_eq!(result.denied, requested);
}

#[test]
fn nothing_approved_denies_everything() {
    let requested = vec![Capability::Clock, net("a.com")];
    let result = grant(&requested, &[]);
    assert!(result.granted.is_empty());
    assert_eq!(result.denied, requested);
}

#[test]
fn approved_path_does_not_cover_sibling_with_shared_prefix() {
    let result = grant(&[fs("/data2/x")], &[fs("/data")]);
    assert!(result.granted.is_empty());
    assert_eq!(result.denied, vec![fs("/data2/x")]);
}

#[test]
fn normalize_rewrites_path_and_host() {
    assert_eq!(fs(r"C:\x\").normalize().unwrap(), fs("C:/x"));
    assert_eq!(
        net("API.Example.com").normalize().unwrap(),
        net("api.example.com")
    );
    assert_eq!(Capability::Clock.normalize().unwrap(), Capability::Clock);
}

#[test]
fn displays_kind_and_value() {
    assert_eq!(fs("/a").to_string(), "fs_read:/a");
    assert_eq!(
        Capability::FsWrite {
            path: "/a".to_owned()
        }
        .to_string(),
        "fs_write:/a"
    );
    assert_eq!(net("a.com").to_string(), "net_host:a.com");
    assert_eq!(Capability::Clock.to_string(), "clock");
}
