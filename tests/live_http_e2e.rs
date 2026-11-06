#![forbid(unsafe_code)]
use ores_otel_api_server::{config::ApiConfig, server};
use std::{io, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

fn config() -> ApiConfig {
    ApiConfig {
        bind: "127.0.0.1:0".to_owned(),
        tcp_bind: None,
        nats_url: None,
    }
}

async fn exchange(address: std::net::SocketAddr, request: &str) -> io::Result<String> {
    let mut client = TcpStream::connect(address).await?;
    client.write_all(request.as_bytes()).await?;
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), client.read_to_end(&mut bytes))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "HTTP read timed out"))??;
    String::from_utf8(bytes).map_err(io::Error::other)
}

async fn get(address: std::net::SocketAddr, path: &str) -> io::Result<String> {
    exchange(
        address,
        &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
    )
    .await
}

#[tokio::test]
async fn live_server_exposes_both_health_routes_and_catalog() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        server::serve_with_listener(listener, config(), async move {
            let _ = shutdown_rx.await;
        })
        .await
    });
    for path in ["/health", "/v1/health"] {
        let response = get(address, path).await.unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.to_ascii_lowercase().contains("application/json"));
        assert!(response.contains(r#""ok":true"#), "{response}");
        assert!(
            response.contains(r#""service":"ores-otel-api-server""#),
            "{response}"
        );
    }
    let catalog = get(address, "/v1").await.unwrap();
    assert!(catalog.starts_with("HTTP/1.1 200"), "{catalog}");
    assert!(
        catalog.contains(r#""resource":"TelemetryRecord""#),
        "{catalog}"
    );
    let post = exchange(address, "POST /v1/health HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await.unwrap();
    assert!(post.starts_with("HTTP/1.1 405"), "{post}");
    let unknown = get(address, "/not-a-route").await.unwrap();
    assert!(unknown.starts_with("HTTP/1.1 404"), "{unknown}");
    shutdown_tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn concurrent_tcp_clients_share_one_live_router() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        server::serve_with_listener(listener, config(), async move {
            let _ = shutdown_rx.await;
        })
        .await
    });
    let mut clients = Vec::new();
    for index in 0..32 {
        clients.push(tokio::spawn(async move {
            let path = if index % 2 == 0 { "/v1/health" } else { "/v1" };
            let response = get(address, path).await.unwrap();
            assert!(response.starts_with("HTTP/1.1 200"), "{response}");
            assert!(response.contains(if index % 2 == 0 {
                "ores-otel-api-server"
            } else {
                "TelemetryRecord"
            }));
        }));
    }
    for client in clients {
        client.await.unwrap();
    }
    shutdown_tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn readonly_routes_survive_head_query_and_rejected_method_over_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        server::serve_with_listener(listener, config(), async move {
            let _ = shutdown_rx.await;
        })
        .await
    });

    let query = get(address, "/v1/health?probe=1").await.unwrap();
    assert!(query.starts_with("HTTP/1.1 200"), "{query}");
    assert!(query.contains(r#""service":"ores-otel-api-server""#));

    let head = exchange(
        address,
        "HEAD /v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .await
    .unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert_eq!(head.split_once("\r\n\r\n").unwrap().1, "");

    for method in ["PUT", "DELETE", "PATCH"] {
        let denied = exchange(
            address,
            &format!("{method} /v1/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
        )
        .await
        .unwrap();
        assert!(denied.starts_with("HTTP/1.1 405"), "{denied}");
    }
    let encoded = get(address, "/v1/health%2Fadmin").await.unwrap();
    assert!(encoded.starts_with("HTTP/1.1 404"), "{encoded}");

    let recovered = get(address, "/v1/health").await.unwrap();
    assert!(recovered.starts_with("HTTP/1.1 200"), "{recovered}");
    shutdown_tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn graceful_shutdown_releases_the_real_tcp_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        server::serve_with_listener(listener, config(), async move {
            let _ = shutdown_rx.await;
        })
        .await
    });
    let live = get(address, "/health").await.unwrap();
    assert!(live.starts_with("HTTP/1.1 200"), "{live}");

    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .expect("graceful server shutdown timed out")
        .unwrap()
        .unwrap();
    assert!(
        TcpStream::connect(address).await.is_err(),
        "server retained an active TCP listener after shutdown"
    );
}
