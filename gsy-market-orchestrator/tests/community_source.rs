use ethers::types::Address;
use gsy_market_orchestrator::community_source::{
    CommunityProvider, OffchainStorageCommunitySource,
};
use gsy_market_orchestrator::config::{Config, OffchainStorageTransport};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn source(url: String) -> OffchainStorageCommunitySource {
    let config = Config {
        evm_node_url: "ws://localhost:8545".to_string(),
        market_controller_address: Address::zero(),
        orchestrator_signer_private_key: String::new(),
        tick_interval_seconds: 1,
        look_ahead_hours: 1,
        offchain_storage_transport: OffchainStorageTransport::Http,
        offchain_storage_url: url,
    };

    OffchainStorageCommunitySource::from_config(&config)
}

fn serve_once(status: &str, body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
    let address = listener.local_addr().unwrap();
    let status = status.to_string();
    let body = body.to_string();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("test request should connect");
        let mut request = [0_u8; 1_024];
        stream
            .read(&mut request)
            .expect("request should be readable");
        write!(
            stream,
            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            body.len(),
            body
        )
        .expect("response should be writable");
    });

    format!("http://{}", address)
}

#[tokio::test]
async fn fetches_all_communities_over_http() {
    let body = r#"[
        {
            "community_id": "11111111-1111-4111-8111-111111111111",
            "community_name": "Community One",
            "sites": ["site-one"]
        },
        {
            "community_id": "22222222-2222-4222-8222-222222222222",
            "community_name": "Community Two",
            "sites": ["site-two"]
        }
    ]"#;
    let source = source(serve_once("200 OK", body));

    let communities = source.fetch_communities().await.unwrap();

    assert_eq!(communities.len(), 2);
    assert_eq!(
        communities[0].community_id,
        "11111111-1111-4111-8111-111111111111"
    );
    assert_eq!(
        communities[1].community_id,
        "22222222-2222-4222-8222-222222222222"
    );
}

#[tokio::test]
async fn propagates_http_failures() {
    let source = source(serve_once(
        "503 Service Unavailable",
        "temporarily unavailable",
    ));

    let error = source.fetch_communities().await.unwrap_err().to_string();

    assert!(error.contains("HTTP 503 Service Unavailable"));
    assert!(error.contains("temporarily unavailable"));
}

#[tokio::test]
async fn propagates_deserialization_failures() {
    let source = source(serve_once("200 OK", r#"{"community_id":"invalid"}"#));

    let error = source.fetch_communities().await.unwrap_err().to_string();

    assert!(error.contains("Failed to deserialize communities"));
}
