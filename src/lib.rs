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
    endpoint: String,
    cookie: String,
}

impl Connection {
    pub fn new_dev(cookie: &str) -> Result<Connection, String> {
        let endpoint = "wss://chat2.strims.gg/ws";
        return Connection::new_at_endpoint(endpoint, cookie);
    }

    pub fn new(cookie: &str) -> Result<Connection, String> {
        let endpoint = "wss://chat.strims.gg/ws";
        return Connection::new_at_endpoint(endpoint, cookie);
    }

    fn new_at_endpoint(endpoint: &str, cookie: &str) -> Result<Connection, String> {
        let socket = Connection::create_socket(&endpoint.to_string(), &cookie.to_string())?;
        Ok(Connection {
            socket,
            endpoint: endpoint.to_string(),
            cookie: cookie.to_string(),
        })
    }

    fn create_socket(
        endpoint: &String,
        cookie: &String,
    ) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, String> {
        let url = Url::parse(&endpoint).expect("Could not parse url");
        let mut request = url.into_client_request().expect("could not build request");
        let cookie = cookie.replace("\n", "");
        request.headers_mut().insert(
            "Cookie",
            HeaderValue::from_str(&cookie).expect("Could not build cookie value"),
        );
        match connect(request) {
            Ok((socket, _)) => Ok(socket),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn send(&mut self, message: &str) -> Result<(), String> {
        let msg = format!("MSG {{\"data\":\"{message}\"}}");
        let res = self.socket.write_message(Message::Text(msg));
        match res {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn read(&mut self) -> Result<String, String> {
        match self.socket.read_message() {
            Ok(m) => Ok(m.to_string()),
            Err(e) => {
                let socket = Connection::create_socket(&self.endpoint, &self.cookie)?;
                self.socket = socket;
                Err(format!("Error reading: [{e}]: => reconnected"))
            }
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
