use std::net::TcpStream;
use tungstenite::{connect, Message};
use url::Url;

fn main() {
    let url = Url::parse("ws://127.0.0.1:9999").unwrap();
    let (mut socket, _response) = connect(url).expect("Failed to connect to WS server");

    // Test 1: echo
    let msg = r#"{"cmd":"echo","message":"hello"}"#;
    socket.send(Message::Text(msg.into())).unwrap();
    let resp = socket.read().unwrap();
    println!("Echo response: {:?}", resp);

    // Test 2: run_test with our E2E test
    let msg = r#"{"cmd":"run_test","file":"tests/runtime/texture_format_change.toml"}"#;
    socket.send(Message::Text(msg.into())).unwrap();
    let resp = socket.read().unwrap();
    println!("Test result: {:?}", resp);
}
