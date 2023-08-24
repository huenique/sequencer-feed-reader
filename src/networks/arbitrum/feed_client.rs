use crate::networks::arbitrum::{
    errors::{ConnectionUpdate, RelayError},
    types::Root,
};
use crossbeam_channel::Sender;
use ethers::providers::StreamExt;
use log::*;
use tokio::{net::TcpStream, task::JoinHandle};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use url::Url;

/// A client for reading transactions from a Sequencer Feed on the Arbitrum network.
pub struct RelayClient {
    /// The WebSocket connection used to read transactions from the feed.
    connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// A channel for sending updates about the connection status (e.g. errors or disconnects).
    connection_update: Sender<ConnectionUpdate>,
    /// A channel for sending transactions received from the feed.
    sender: Sender<Root>,
    /// The ID of the relay that this client is connected to.
    id: u32,
}

impl RelayClient {
    /// Creates a new `FeedClient` instance.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the websocket server to connect to.
    /// * `chain_id` - The expected chain ID of the server.
    /// * `id` - The ID of this client instance.
    /// * `sender` - The sender channel for sending `Root` messages.
    /// * `connection_update` - The sender channel for sending `ConnectionUpdate` messages.
    ///
    /// # Returns
    ///
    /// A `Result` containing the new `FeedClient` instance, or a `RelayError` if an error occurred.
    pub async fn new(
        url: Url,
        chain_id: u64,
        id: u32,
        sender: Sender<Root>,
        connection_update: Sender<ConnectionUpdate>,
    ) -> Result<Self, RelayError> {
        let req = generate_websocket_request(url)?;
        let (socket, resp) = connect_async(req).await?;
        check_chain_id_header(resp, chain_id)?;

        Ok(Self {
            connection: socket,
            connection_update,
            sender,
            id,
        })
    }

    /// Spawns a new Tokio task to run the feed client.
    ///
    /// # Returns
    ///
    /// A `JoinHandle` that can be used to await the completion of the spawned task.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            match self.run().await {
                Ok(_) => (),
                Err(e) => error!("{}", e),
            }
        })
    }

    pub async fn run(mut self) -> Result<(), RelayError> {
        while let Some(msg) = self.connection.next().await {
            match msg {
                Ok(message) => {
                    let decoded_root: Root = match serde_json::from_slice(&message.into_data()) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };

                    if self.sender.send(decoded_root).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    self.connection_update
                        .send(ConnectionUpdate::StoppedSendingFrames(self.id))?;
                    error!("Connection closed with error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}

/// Checks if the `arbitrum-chain-id` header in the response matches the expected chain ID.
///
/// # Arguments
///
/// * `resp` - The HTTP response from the server.
/// * `chain_id` - The expected chain ID.
///
/// # Errors
///
/// Returns a `RelayError::InvalidChainId` error if the `arbitrum-chain-id` header is missing or
/// does not match the expected chain ID.
///
/// # Examples
///
/// ```
/// use tungstenite::http::Response;
/// use sequencer_feed_reader::networks::arbitrum::feed_client::check_chain_id_header;
///
/// let resp = Response::builder()
///     .header("arbitrum-chain-id", "123")
///     .body(None)
///     .unwrap();
///
/// // This should return Ok(())
/// assert_eq!(check_chain_id_header(resp, 123), Ok(()));
///
/// // This should return Err(RelayError::InvalidChainId)
/// assert!(check_chain_id_header(resp, 456).is_err());
/// ```
fn check_chain_id_header(
    resp: tungstenite::http::Response<Option<Vec<u8>>>,
    chain_id: u64,
) -> Result<(), RelayError> {
    let chain_id_resp = resp
        .headers()
        .get("arbitrum-chain-id")
        .ok_or(RelayError::InvalidChainId)?
        .to_str()
        .unwrap_or_default();
    Ok(
        if chain_id_resp.parse::<u64>().unwrap_or_default() != chain_id {
            return Err(RelayError::InvalidChainId);
        },
    )
}

/// Generates a WebSocket request for the given URL.
///
/// # Arguments
///
/// * `url` - The URL to generate the request for.
///
/// # Examples
///
/// ```
/// use url::Url;
/// use sequencer_feed_reader::networks::arbitrum::feed_client::generate_websocket_request;
///
/// let url = Url::parse("wss://example.com").unwrap();
/// let request = generate_websocket_request(url).unwrap();
///
/// assert_eq!(request.method(), "GET");
/// assert_eq!(request.uri().to_string(), "wss://example.com/");
/// assert_eq!(request.headers()["Host"], "example.com");
/// assert_eq!(request.headers()["Connection"], "Upgrade");
/// assert_eq!(request.headers()["Upgrade"], "websocket");
/// assert_eq!(request.headers()["Sec-WebSocket-Version"], "13");
/// assert!(request.headers()["Sec-WebSocket-Key"].len() > 0);
/// assert_eq!(request.headers()["Arbitrum-Feed-Client-Version"], "2");
/// assert_eq!(request.headers()["Arbitrum-Requested-Sequence-number"], "0");
/// ```
///
/// # Returns
///
/// Returns a `Result` containing the generated WebSocket request if successful, or a `RelayError` if an error occurred.
fn generate_websocket_request(url: Url) -> Result<tungstenite::http::Request<()>, RelayError> {
    let key = tungstenite::handshake::client::generate_key();
    let host = url.host_str().ok_or(RelayError::InvalidUrl)?;
    let req = tungstenite::handshake::client::Request::builder()
        .method("GET")
        .uri(url.as_str())
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", key)
        .header("Arbitrum-Feed-Client-Version", "2")
        .header("Arbitrum-Requested-Sequence-number", "0")
        .body(())?;
    Ok(req)
}
