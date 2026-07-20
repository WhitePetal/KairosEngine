use crate::kairos_test_harness::{
    bridge::EngineCommand,
    test_runner,
    types::{WsRequest, WsResponse},
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

/// Start the WebSocket server on the given port.
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
                let response = handle_message(&text, &sender).await;
                let json = serde_json::to_string(&response).unwrap_or_else(|_| {
                    r#"{"status":"error","error":"serialization failed"}"#.into()
                });
                if ws_stream.send(Message::Text(json.into())).await.is_err() {
                    break;
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

async fn handle_message(text: &str, sender: &mpsc::Sender<EngineCommand>) -> WsResponse {
    let request: WsRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(_) => {
            // Fallback: treat as legacy echo
            let (tx, rx) = oneshot::channel();
            if sender
                .send(EngineCommand::Echo {
                    message: text.to_string(),
                    response: tx,
                })
                .await
                .is_err()
            {
                return WsResponse::error("bridge channel closed");
            }
            return match rx.await {
                Ok(echoed) => WsResponse::echo(echoed),
                Err(_) => WsResponse::error("engine dropped response"),
            };
        }
    };

    match request.cmd.as_str() {
        "run_test" => {
            let file = match &request.file {
                Some(f) if !f.is_empty() => f.clone(),
                _ => return WsResponse::error("missing 'file' field"),
            };

            let sender = sender.clone();
            let (tx, rx) = oneshot::channel();

            tokio::spawn(async move {
                let result = test_runner::run_test_file(&file, sender).await;
                let _ = tx.send(result);
            });

            match rx.await {
                Ok(result) => WsResponse::from_test_result(result),
                Err(_) => WsResponse::error("test runner task panicked"),
            }
        }
        "echo" => {
            let (tx, rx) = oneshot::channel();
            if sender
                .send(EngineCommand::Echo {
                    message: text.to_string(),
                    response: tx,
                })
                .await
                .is_err()
            {
                return WsResponse::error("bridge channel closed");
            }
            match rx.await {
                Ok(echoed) => WsResponse::echo(echoed),
                Err(_) => WsResponse::error("engine dropped response"),
            }
        }
        other => WsResponse::error(format!("unknown command: '{other}'")),
    }
}
