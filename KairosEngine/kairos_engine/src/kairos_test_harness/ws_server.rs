use crate::kairos_test_harness::bridge::EngineCommand;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

/// Start the WebSocket server on the given port.
///
/// Spawns a background task for each accepted connection.
/// The server runs until the tokio runtime is dropped.
pub async fn start(sender: mpsc::Sender<EngineCommand>, port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("Test harness WS server failed to bind {addr}: {e}");
            return;
        }
    };

    log::info!("Test harness WS server listening on ws://{addr}");

    while let Ok((stream, peer_addr)) = listener.accept().await {
        log::debug!("WS connection from {peer_addr}");

        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                log::warn!("WS handshake failed: {e}");
                continue;
            }
        };

        let sender = sender.clone();
        tokio::spawn(handle_connection(ws_stream, sender));
    }
}

async fn handle_connection(
    mut ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    sender: mpsc::Sender<EngineCommand>,
) {
    while let Some(msg) = ws_stream.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                log::warn!("WS read error: {e}");
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                let text = text.to_string();
                let (response_tx, response_rx) = oneshot::channel();

                if sender
                    .send(EngineCommand::Echo {
                        message: text,
                        response: response_tx,
                    })
                    .await
                    .is_err()
                {
                    log::warn!("Bridge channel closed, dropping WS connection");
                    break;
                }

                match response_rx.await {
                    Ok(response) => {
                        if ws_stream.send(Message::Text(response.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        log::warn!("Engine dropped the oneshot sender before responding");
                    }
                }
            }
            Message::Close(_) => break,
            Message::Ping(data) => {
                let _ = ws_stream.send(Message::Pong(data)).await;
            }
            _ => {}
        }
    }
}
