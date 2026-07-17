//! Futu/Moomoo OpenD raw TCP session transport.
//!
//! This module implements the official fixed-size OpenD frame and the session
//! bootstrap protocols needed before a paper order can be submitted. Order
//! conversion and submission deliberately remain in the next adapter PR.

use std::{fmt, time::Duration};

use serde_json::{json, Map, Value};
use sha1::{Digest, Sha1};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use crate::{BrokerEnvironment, OpenDConnectionConfig};

const HEADER_LENGTH: usize = 44;
const MAX_BODY_LENGTH: usize = 1024 * 1024;
const JSON_FORMAT: u8 = 1;
const INIT_CONNECT_PROTOCOL_ID: u32 = 1001;
const GET_GLOBAL_STATE_PROTOCOL_ID: u32 = 1002;
const GET_ACCOUNT_LIST_PROTOCOL_ID: u32 = 2001;
const PAPER_TRADING_ENVIRONMENT: i64 = 0;
const SESSION_TIMEOUT: Duration = Duration::from_secs(5);
const CLIENT_ID: &str = "indexlink-rust";
const CLIENT_VERSION: i64 = 1;

/// Session bootstrap failure for a raw OpenD paper connection.
///
/// Error strings intentionally do not reveal host names, account IDs, packet
/// contents, credentials, or provider error messages.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OpenDSessionError {
    /// The session was requested for anything other than paper trading.
    #[error("opend paper session requires Paper environment")]
    PaperTradingRequired,
    /// The raw transport was configured for a host other than local OpenD.
    #[error("opend raw TCP transport requires a loopback host")]
    RemoteHostUnsupported,
    /// Connecting to or exchanging a packet with OpenD timed out.
    #[error("opend session request timed out")]
    Timeout,
    /// OpenD could not be reached or closed the TCP connection.
    #[error("opend is unavailable")]
    Unavailable,
    /// OpenD returned a malformed or mismatched protocol frame.
    #[error("opend returned an invalid protocol response")]
    InvalidResponse,
    /// OpenD rejected a bootstrap request.
    #[error("opend rejected the session request")]
    Rejected,
    /// OpenD is reachable but has not authenticated its trading service.
    #[error("opend trading service is not authenticated")]
    TradingNotLoggedIn,
    /// OpenD did not return any simulated trading account.
    #[error("opend did not return a paper trading account")]
    NoPaperAccount,
    /// The configured paper account was absent or was not simulated.
    #[error("configured opend paper account was not available")]
    ConfiguredPaperAccountNotFound,
    /// More than one paper account is available without an explicit selection.
    #[error("multiple opend paper accounts are available; configure an account id")]
    AmbiguousPaperAccounts,
}

/// Connected, initialized, and paper-account-selected OpenD TCP session.
///
/// This transport is deliberately limited to a loopback OpenD instance. The
/// official protocol supports optional packet encryption, but that RSA setup is
/// outside this PR; refusing a remote plaintext TCP connection is safer than
/// silently forwarding trading metadata across the network.
pub struct OpenDPaperSession {
    // Retained so the initialized socket stays open for the order gateway in PR 2.
    _transport: OpenDTcpTransport,
    connection_id: u64,
    selected_account_id: String,
}

impl OpenDPaperSession {
    /// Connect to local OpenD, initialize a JSON session, verify trade login,
    /// and select one simulated account.
    ///
    /// If [`OpenDConnectionConfig::account_id`] is set, it must match a
    /// simulated account returned by OpenD. Without an explicit account ID,
    /// exactly one simulated account must be available.
    pub async fn connect(config: &OpenDConnectionConfig) -> Result<Self, OpenDSessionError> {
        if config.environment() != BrokerEnvironment::Paper || config.live_trading_enabled() {
            return Err(OpenDSessionError::PaperTradingRequired);
        }
        if !is_loopback_host(config.host()) {
            return Err(OpenDSessionError::RemoteHostUnsupported);
        }

        let mut transport = OpenDTcpTransport::connect(config).await?;
        let connection_id = initialize_connection(&mut transport).await?;
        verify_trading_login(&mut transport).await?;
        let accounts = fetch_paper_accounts(&mut transport).await?;
        let selected_account_id = select_paper_account(config.account_id(), accounts)?;

        Ok(Self {
            _transport: transport,
            connection_id,
            selected_account_id,
        })
    }

    /// Return the OpenD connection ID assigned during `InitConnect`.
    #[must_use]
    pub fn connection_id(&self) -> u64 {
        self.connection_id
    }

    /// Return the explicitly selected simulated account ID.
    ///
    /// Callers must treat this identifier as sensitive operational metadata and
    /// must not place it in HTTP responses, audit snapshots, or unredacted logs.
    #[must_use]
    pub fn selected_account_id(&self) -> &str {
        &self.selected_account_id
    }
}

impl fmt::Debug for OpenDPaperSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenDPaperSession")
            .field("connection_id", &self.connection_id)
            .field("selected_account_id", &"<redacted>")
            .finish_non_exhaustive()
    }
}

struct OpenDTcpTransport {
    stream: TcpStream,
    next_serial: u32,
}

impl OpenDTcpTransport {
    async fn connect(config: &OpenDConnectionConfig) -> Result<Self, OpenDSessionError> {
        let address = socket_address(config.host(), config.port());
        let stream = timeout(SESSION_TIMEOUT, TcpStream::connect(address))
            .await
            .map_err(|_| OpenDSessionError::Timeout)?
            .map_err(|_| OpenDSessionError::Unavailable)?;

        Ok(Self {
            stream,
            next_serial: 1,
        })
    }

    async fn request(&mut self, protocol_id: u32, body: Value) -> Result<Value, OpenDSessionError> {
        let serial = self.next_serial;
        self.next_serial = self
            .next_serial
            .checked_add(1)
            .ok_or(OpenDSessionError::InvalidResponse)?;
        let body = serde_json::to_vec(&body).map_err(|_| OpenDSessionError::InvalidResponse)?;
        let request = encode_frame(protocol_id, serial, &body);

        let response = timeout(SESSION_TIMEOUT, async {
            self.stream
                .write_all(&request)
                .await
                .map_err(|_| OpenDSessionError::Unavailable)?;
            self.stream
                .flush()
                .await
                .map_err(|_| OpenDSessionError::Unavailable)?;
            read_frame(&mut self.stream).await
        })
        .await
        .map_err(|_| OpenDSessionError::Timeout)??;

        if response.protocol_id != protocol_id
            || response.serial != serial
            || response.format != JSON_FORMAT
        {
            return Err(OpenDSessionError::InvalidResponse);
        }

        serde_json::from_slice(&response.body).map_err(|_| OpenDSessionError::InvalidResponse)
    }
}

struct OpenDFrame {
    protocol_id: u32,
    format: u8,
    serial: u32,
    body: Vec<u8>,
}

async fn initialize_connection(
    transport: &mut OpenDTcpTransport,
) -> Result<u64, OpenDSessionError> {
    let response = transport
        .request(
            INIT_CONNECT_PROTOCOL_ID,
            json!({
                "c2s": {
                    "clientVer": CLIENT_VERSION,
                    "clientID": CLIENT_ID,
                    "recvNotify": false,
                    "packetEncAlgo": 0,
                    "pushProtoFmt": JSON_FORMAT,
                }
            }),
        )
        .await?;
    let payload = successful_payload(&response)?;
    u64_field(payload, "connID")
}

async fn verify_trading_login(transport: &mut OpenDTcpTransport) -> Result<(), OpenDSessionError> {
    let response = transport
        .request(
            GET_GLOBAL_STATE_PROTOCOL_ID,
            json!({"c2s": {"userID": "0"}}),
        )
        .await?;
    let payload = successful_payload(&response)?;
    match payload.get("trdLogined").and_then(Value::as_bool) {
        Some(true) => Ok(()),
        Some(false) => Err(OpenDSessionError::TradingNotLoggedIn),
        None => Err(OpenDSessionError::InvalidResponse),
    }
}

async fn fetch_paper_accounts(
    transport: &mut OpenDTcpTransport,
) -> Result<Vec<String>, OpenDSessionError> {
    let response = transport
        .request(
            GET_ACCOUNT_LIST_PROTOCOL_ID,
            json!({
                "c2s": {
                    "userID": "0",
                    "needGeneralSecAccount": true,
                }
            }),
        )
        .await?;
    let payload = successful_payload(&response)?;
    let accounts = payload
        .get("accList")
        .and_then(Value::as_array)
        .ok_or(OpenDSessionError::InvalidResponse)?;

    let mut paper_accounts = Vec::new();
    for account in accounts {
        let account = account
            .as_object()
            .ok_or(OpenDSessionError::InvalidResponse)?;
        if integer_field(account, "trdEnv")? == PAPER_TRADING_ENVIRONMENT {
            paper_accounts.push(string_field(account, "accID")?);
        }
    }
    Ok(paper_accounts)
}

fn select_paper_account(
    configured_account_id: Option<&str>,
    accounts: Vec<String>,
) -> Result<String, OpenDSessionError> {
    if let Some(configured_account_id) = configured_account_id {
        return accounts
            .into_iter()
            .find(|account_id| account_id == configured_account_id)
            .ok_or(OpenDSessionError::ConfiguredPaperAccountNotFound);
    }

    match accounts.len() {
        0 => Err(OpenDSessionError::NoPaperAccount),
        1 => Ok(accounts.into_iter().next().expect("length checked")),
        _ => Err(OpenDSessionError::AmbiguousPaperAccounts),
    }
}

fn successful_payload(response: &Value) -> Result<&Map<String, Value>, OpenDSessionError> {
    match integer_value(response.get("retType")) {
        Some(0) => {}
        Some(_) => return Err(OpenDSessionError::Rejected),
        None => return Err(OpenDSessionError::InvalidResponse),
    }

    response
        .get("s2c")
        .and_then(Value::as_object)
        .ok_or(OpenDSessionError::InvalidResponse)
}

fn u64_field(payload: &Map<String, Value>, name: &str) -> Result<u64, OpenDSessionError> {
    let value = payload
        .get(name)
        .ok_or(OpenDSessionError::InvalidResponse)?;
    match value {
        Value::Number(number) => number.as_u64().ok_or(OpenDSessionError::InvalidResponse),
        Value::String(value) => value
            .parse::<u64>()
            .map_err(|_| OpenDSessionError::InvalidResponse),
        _ => Err(OpenDSessionError::InvalidResponse),
    }
}

fn integer_field(payload: &Map<String, Value>, name: &str) -> Result<i64, OpenDSessionError> {
    integer_value(payload.get(name)).ok_or(OpenDSessionError::InvalidResponse)
}

fn integer_value(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number.as_i64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn string_field(payload: &Map<String, Value>, name: &str) -> Result<String, OpenDSessionError> {
    let value = payload
        .get(name)
        .ok_or(OpenDSessionError::InvalidResponse)?;
    match value {
        Value::String(value) if !value.is_empty() => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        _ => Err(OpenDSessionError::InvalidResponse),
    }
}

fn is_loopback_host(host: &str) -> bool {
    host == "127.0.0.1" || host == "::1" || host.eq_ignore_ascii_case("localhost")
}

fn socket_address(host: &str, port: u16) -> String {
    if host.eq_ignore_ascii_case("localhost") {
        format!("127.0.0.1:{port}")
    } else if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn encode_frame(protocol_id: u32, serial: u32, body: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LENGTH + body.len());
    let body_hash = Sha1::digest(body);
    frame.extend_from_slice(b"FT");
    frame.extend_from_slice(&protocol_id.to_le_bytes());
    frame.push(JSON_FORMAT);
    frame.push(0);
    frame.extend_from_slice(&serial.to_le_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body_hash);
    frame.extend_from_slice(&[0; 8]);
    frame.extend_from_slice(body);
    frame
}

async fn read_frame(stream: &mut TcpStream) -> Result<OpenDFrame, OpenDSessionError> {
    let mut header = [0_u8; HEADER_LENGTH];
    stream
        .read_exact(&mut header)
        .await
        .map_err(|_| OpenDSessionError::Unavailable)?;
    if header[..2] != *b"FT" {
        return Err(OpenDSessionError::InvalidResponse);
    }

    let protocol_id = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
    let format = header[6];
    let serial = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    let body_length = u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;
    if body_length > MAX_BODY_LENGTH {
        return Err(OpenDSessionError::InvalidResponse);
    }

    let mut body = vec![0_u8; body_length];
    stream
        .read_exact(&mut body)
        .await
        .map_err(|_| OpenDSessionError::Unavailable)?;
    if header[7] != 0 || Sha1::digest(&body).as_slice() != &header[16..36] {
        return Err(OpenDSessionError::InvalidResponse);
    }

    Ok(OpenDFrame {
        protocol_id,
        format,
        serial,
        body,
    })
}

#[cfg(test)]
mod tests {
    use tokio::{io::AsyncWriteExt, net::TcpListener};

    use super::*;
    use crate::{BrokerProvider, OpenDConnectionConfig};

    /// Start a local fake that verifies session requests and returns supplied state.
    async fn spawn_opend(
        trading_logged_in: bool,
        accounts: Vec<Value>,
    ) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("client should connect");

            let init = read_frame(&mut stream).await.expect("valid init frame");
            assert_eq!(init.protocol_id, INIT_CONNECT_PROTOCOL_ID);
            assert_eq!(init.serial, 1);
            assert_eq!(
                serde_json::from_slice::<Value>(&init.body).unwrap(),
                json!({
                    "c2s": {
                        "clientVer": CLIENT_VERSION,
                        "clientID": CLIENT_ID,
                        "recvNotify": false,
                        "packetEncAlgo": 0,
                        "pushProtoFmt": JSON_FORMAT,
                    }
                })
            );
            stream
                .write_all(&encode_frame(
                    INIT_CONNECT_PROTOCOL_ID,
                    init.serial,
                    &serde_json::to_vec(&json!({"retType": 0, "s2c": {"connID": "42"}})).unwrap(),
                ))
                .await
                .unwrap();

            let state = read_frame(&mut stream).await.expect("valid state frame");
            assert_eq!(state.protocol_id, GET_GLOBAL_STATE_PROTOCOL_ID);
            assert_eq!(state.serial, 2);
            stream
                .write_all(&encode_frame(
                    GET_GLOBAL_STATE_PROTOCOL_ID,
                    state.serial,
                    &serde_json::to_vec(
                        &json!({"retType": 0, "s2c": {"trdLogined": trading_logged_in}}),
                    )
                    .unwrap(),
                ))
                .await
                .unwrap();

            if !trading_logged_in {
                return;
            }

            let account_list = read_frame(&mut stream)
                .await
                .expect("valid account-list frame");
            assert_eq!(account_list.protocol_id, GET_ACCOUNT_LIST_PROTOCOL_ID);
            assert_eq!(account_list.serial, 3);
            stream
                .write_all(&encode_frame(
                    GET_ACCOUNT_LIST_PROTOCOL_ID,
                    account_list.serial,
                    &serde_json::to_vec(&json!({"retType": 0, "s2c": {"accList": accounts}}))
                        .unwrap(),
                ))
                .await
                .unwrap();
        });

        (port, task)
    }

    /// Verify the official raw frames initialize and select the sole paper account.
    #[tokio::test]
    async fn session_initializes_and_selects_single_paper_account() {
        let (port, server) = spawn_opend(
            true,
            vec![
                json!({"trdEnv": 1, "accID": "live-account"}),
                json!({"trdEnv": 0, "accID": "paper-account"}),
            ],
        )
        .await;
        let config = OpenDConnectionConfig::paper(BrokerProvider::Futu, "127.0.0.1", port)
            .expect("paper config should be valid");

        let session = OpenDPaperSession::connect(&config)
            .await
            .expect("paper session should initialize");

        assert_eq!(session.connection_id(), 42);
        assert_eq!(session.selected_account_id(), "paper-account");
        assert!(!format!("{session:?}").contains("paper-account"));
        server.await.unwrap();
    }

    /// Verify an explicit configuration chooses the matching paper account.
    #[tokio::test]
    async fn session_uses_explicit_paper_account_selection() {
        let (port, server) = spawn_opend(
            true,
            vec![
                json!({"trdEnv": 0, "accID": "paper-a"}),
                json!({"trdEnv": 0, "accID": "paper-b"}),
            ],
        )
        .await;
        let config = OpenDConnectionConfig::paper_with_account(
            BrokerProvider::Moomoo,
            "localhost",
            port,
            "paper-b",
        )
        .expect("paper config should be valid");

        let session = OpenDPaperSession::connect(&config)
            .await
            .expect("configured paper account should be selected");

        assert_eq!(session.selected_account_id(), "paper-b");
        server.await.unwrap();
    }

    /// Verify a non-authenticated OpenD trading service cannot form a session.
    #[tokio::test]
    async fn session_rejects_opend_without_trade_login() {
        let (port, server) = spawn_opend(false, Vec::new()).await;
        let config = OpenDConnectionConfig::paper(BrokerProvider::Futu, "127.0.0.1", port)
            .expect("paper config should be valid");

        assert!(matches!(
            OpenDPaperSession::connect(&config).await,
            Err(OpenDSessionError::TradingNotLoggedIn)
        ));
        server.await.unwrap();
    }

    /// Verify malformed simulated-account payloads never become an implicit selection.
    #[tokio::test]
    async fn session_rejects_malformed_paper_account() {
        let (port, server) = spawn_opend(true, vec![json!({"trdEnv": 0})]).await;
        let config = OpenDConnectionConfig::paper(BrokerProvider::Futu, "127.0.0.1", port)
            .expect("paper config should be valid");

        assert!(matches!(
            OpenDPaperSession::connect(&config).await,
            Err(OpenDSessionError::InvalidResponse)
        ));
        server.await.unwrap();
    }

    /// Verify raw TCP never attempts a non-loopback plaintext OpenD connection.
    #[tokio::test]
    async fn session_rejects_remote_raw_tcp_host() {
        let config = OpenDConnectionConfig::paper(BrokerProvider::Futu, "opend.example", 11111)
            .expect("paper config should be valid");

        assert!(matches!(
            OpenDPaperSession::connect(&config).await,
            Err(OpenDSessionError::RemoteHostUnsupported)
        ));
    }

    /// Verify account selection never falls back to a live account or an ambiguous paper account.
    #[test]
    fn paper_account_selection_requires_one_safe_candidate() {
        assert_eq!(
            select_paper_account(None, Vec::new()),
            Err(OpenDSessionError::NoPaperAccount)
        );
        assert_eq!(
            select_paper_account(None, vec!["paper-a".to_owned(), "paper-b".to_owned()]),
            Err(OpenDSessionError::AmbiguousPaperAccounts)
        );
        assert_eq!(
            select_paper_account(Some("paper-a"), vec!["paper-b".to_owned()]),
            Err(OpenDSessionError::ConfiguredPaperAccountNotFound)
        );
    }

    /// Verify localhost always resolves to a literal loopback socket address.
    #[test]
    fn localhost_uses_a_literal_loopback_address() {
        assert_eq!(socket_address("localhost", 11111), "127.0.0.1:11111");
        assert_eq!(socket_address("LOCALHOST", 11111), "127.0.0.1:11111");
    }
}
