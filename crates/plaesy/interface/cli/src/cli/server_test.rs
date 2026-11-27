use super::*;
use std::net::Ipv4Addr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn bind_layer(bind: &str) -> Layer {
    Layer {
        bind: Some(bind.to_owned()),
        ..Layer::default()
    }
}

fn opted_in(bind: &str) -> Layer {
    Layer {
        bind: Some(bind.to_owned()),
        allow_non_loopback: Some(true),
        tls_terminated_upstream: Some(true),
        ..Layer::default()
    }
}

#[test]
fn default_bind_is_loopback_8790() {
    let config = resolve(Layer::default(), Layer::default(), Layer::default()).expect("defaults");
    assert_eq!(config.bind, "127.0.0.1:8790".parse().expect("addr"));
    assert_eq!(config.db, PathBuf::from(DEFAULT_DB));
    assert!(check_bind(&config, false).is_ok());
}

#[test]
fn flags_override_environment_which_overrides_file() {
    let file = bind_layer("127.0.0.1:9001");
    let env = bind_layer("127.0.0.1:9002");
    let flags = bind_layer("127.0.0.1:9003");
    let config = resolve(file.clone(), env.clone(), flags).expect("flags win");
    assert_eq!(config.bind.port(), 9003);
    let config = resolve(file, env, Layer::default()).expect("env wins over file");
    assert_eq!(config.bind.port(), 9002);
}

#[test]
fn file_keeps_keys_that_higher_layers_do_not_set() {
    let file = Layer {
        db: Some(PathBuf::from("from-file.sqlite")),
        ..Layer::default()
    };
    let config = resolve(file, bind_layer("127.0.0.1:9004"), Layer::default()).expect("merge");
    assert_eq!(config.db, PathBuf::from("from-file.sqlite"));
    assert_eq!(config.bind.port(), 9004);
}

#[test]
fn environment_reads_prefixed_keys_and_skips_empty_values() {
    let env = layer_from_env(|name| match name {
        "PLAESY_SERVER_BIND" => Some("127.0.0.1:9100".to_owned()),
        "PLAESY_SERVER_DB" => Some(String::new()),
        "PLAESY_SERVER_ALLOW_NON_LOOPBACK" => Some("true".to_owned()),
        "PLAESY_SERVER_TLS_TERMINATED_UPSTREAM" => Some("0".to_owned()),
        "PLAESY_SERVER_CORS_ORIGINS" => Some("https://a.example, http://localhost:5173 ,".to_owned()),
        _ => None,
    })
    .expect("env");
    assert_eq!(env.bind.as_deref(), Some("127.0.0.1:9100"));
    assert_eq!(env.db, None);
    assert_eq!(env.allow_non_loopback, Some(true));
    assert_eq!(env.tls_terminated_upstream, Some(false));
    assert_eq!(
        env.cors_origins,
        Some(vec![
            "https://a.example".to_owned(),
            "http://localhost:5173".to_owned()
        ])
    );
}

#[test]
fn environment_rejects_a_non_boolean_switch() {
    let error = layer_from_env(|name| {
        (name == "PLAESY_SERVER_ALLOW_NON_LOOPBACK").then(|| "yes".to_owned())
    })
    .expect_err("yes is not a boolean");
    assert!(matches!(error, ConfigError::BadFlag { .. }));
}

#[test]
fn non_loopback_bind_is_refused_without_opt_in() {
    let config = resolve(
        Layer::default(),
        Layer::default(),
        bind_layer("0.0.0.0:8790"),
    )
    .expect("parses");
    assert_eq!(check_bind(&config, false), Err(ConfigError::NonLoopbackRefused));
    assert_eq!(check_bind(&config, true), Err(ConfigError::NonLoopbackRefused));
}

#[test]
fn non_loopback_bind_needs_authentication_even_with_both_opt_ins() {
    let config = resolve(Layer::default(), Layer::default(), opted_in("0.0.0.0:8790"))
        .expect("parses");
    assert_eq!(check_bind(&config, false), Err(ConfigError::AuthRequired));
    assert!(check_bind(&config, true).is_ok());
}

#[test]
fn only_one_opt_in_is_not_enough() {
    let only_flag = Layer {
        allow_non_loopback: Some(true),
        ..bind_layer("0.0.0.0:8790")
    };
    let config = resolve(Layer::default(), Layer::default(), only_flag).expect("parses");
    assert_eq!(check_bind(&config, true), Err(ConfigError::NonLoopbackRefused));
}

#[test]
fn cors_origins_must_be_exact_origins() {
    assert!(resolve(
        Layer::default(),
        Layer::default(),
        Layer {
            cors_origins: Some(vec!["https://app.example".to_owned()]),
            ..Layer::default()
        }
    )
    .is_ok());
    for bad in ["*", "https://app.example/x", "app.example", "https://*.example", "https://"] {
        let result = resolve(
            Layer::default(),
            Layer::default(),
            Layer {
                cors_origins: Some(vec![bad.to_owned()]),
                ..Layer::default()
            },
        );
        assert!(
            matches!(result, Err(ConfigError::BadOrigin(_))),
            "{bad} must be refused"
        );
    }
}

#[test]
fn bad_bind_text_is_reported() {
    let result = resolve(Layer::default(), Layer::default(), bind_layer("localhost"));
    assert_eq!(result, Err(ConfigError::BadBind("localhost".to_owned())));
}

#[test]
fn unknown_file_keys_are_rejected() {
    let parsed = toml::from_str::<Layer>("bnd = \"127.0.0.1:1\"\n");
    assert!(parsed.is_err());
}

#[tokio::test]
async fn serve_answers_health_and_stops_on_shutdown() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open_store(&dir.path().join("docs.sqlite")).expect("store");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let address = listener.local_addr().expect("address");
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(serve_on(listener, store, async {
        let _ = stopped.await;
    }));

    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect");
    stream
        .write_all(b"GET /v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("write");
    let mut response = String::new();
    stream.read_to_string(&mut response).await.expect("read");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains("\"ok\":true"), "{response}");

    stop.send(()).expect("shutdown signal");
    server.await.expect("join").expect("serve returns cleanly");
}
