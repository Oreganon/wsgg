use serde_json::Value;
use std::net::TcpStream;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};
use url::Url;

#[derive(Debug)]
pub struct Connection {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl Connection {
    pub fn new(cookie: &str) -> Result<Connection, &'static str> {
        let endpoint = "wss://chat.strims.gg/ws";
        let url = Url::parse(endpoint).expect("Could not parse url");
        let mut request = url.into_client_request().expect("could not build request");
        request.headers_mut().insert(
            "Cookie",
            HeaderValue::from_str(cookie).expect("Could not build cookie value"),
        );
        let (socket, _) = connect(request).unwrap();
        Ok(Connection { socket })
    }

    pub fn send(&mut self, message: &str) -> Result<(), &'static str> {
        let msg = format!("MSG {{\"data\":\"{message}\"}}");
        self.socket.write_message(Message::Text(msg)).unwrap();
        Ok(())
    }

    fn read(&mut self) -> Result<String, String> {
        match self.socket.read_message() {
            Ok(m) => Ok(m.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn read_msg(&mut self) -> Result<Value, String> {
        loop {
            let msg = self.read()?;
            let prefix = "MSG ";
            if msg.starts_with(prefix) {
                let msg = msg.strip_prefix(prefix).expect("could not strip prefix");
                match serde_json::from_str(msg) {
                    Ok(v) => return Ok(v),
                    Err(e) => return Err(e.to_string()),
                }
            }
        }
    }
}
