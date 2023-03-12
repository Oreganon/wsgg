use reqwest::header;
use reqwest::Url;
use serde_json::Value;
use std::net::TcpStream;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};

#[derive(Debug)]
pub struct Connection {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    endpoint: String,
    cookie: String,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub sender: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct WhisperMessage {
    pub sender: String,
    pub receiver: String,
    pub message: String,
}

#[derive(Debug)]
pub enum MessageType {
    ChatMessage(ChatMessage),
    WhisperMessage(WhisperMessage),
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

    fn verify_cookie(cookie: String) -> Result<(), String> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "Cookie",
            header::HeaderValue::from_str(&cookie)
                .map_err(|e| format!("{e}: [cookie=\"{cookie}\""))?,
        );

        let client = reqwest::blocking::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| e.to_string())?;
        let url = "https://chat.strims.gg/api/chat/me";
        let res = client.get(url).send().map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let msg = format!("Could not log in with cookie. The cookie must have the format as specified in the cookie.template file. [cookie=\"{cookie}\"]");
            return Err(msg);
        }
        Ok(())
    }

    fn create_socket(
        endpoint: &String,
        cookie: &String,
    ) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, String> {
        Connection::verify_cookie(cookie.to_string())?;
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
        self.write(msg)
    }

    pub fn whisper(&mut self, receiver: &str, message: &str) -> Result<(), String> {
        let msg = format!("PRIVMSG {{\"nick\":\"{receiver}\",\"data\":\"{message}\"}}");
        self.write(msg)
    }

    fn write(&mut self, text: String) -> Result<(), String> {
        let res = self.socket.write_message(Message::Text(text));
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

    fn read_until_prefix(&mut self, prefix: &str) -> Result<String, String> {
        loop {
            let msg = self.read()?;
            if msg.starts_with(prefix) {
                let msg = msg.strip_prefix(prefix).expect("could not strip prefix");
                return Ok(msg.to_owned());
            }
        }
    }

    fn msg_to_chatmessage(msg: String) -> Result<ChatMessage, String> {
        let v: Result<Value, _> = serde_json::from_str(&msg);
        match v {
            Ok(v) => {
                let message = v["data"].to_string();
                let sender = v["nick"].to_string();
                return Ok(ChatMessage {
                    message: clean_received(&message).to_owned(),
                    sender: clean_received(&sender).to_owned(),
                });
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    fn msg_to_whispermessage(msg: String) -> Result<WhisperMessage, String> {
        let v: Result<Value, _> = serde_json::from_str(&msg);
        match v {
            Ok(v) => {
                let message = v["data"].to_string();
                let sender = v["nick"].to_string();
                let receiver = v["nickTarget"].to_string();
                return Ok(WhisperMessage {
                    message: clean_received(&message).to_owned(),
                    sender: clean_received(&sender).to_owned(),
                    receiver: clean_received(&receiver).to_owned(),
                });
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    pub fn read_all(&mut self) -> Result<MessageType, String> {
        loop {
            let msg = self.read()?;
            let prefix = "MSG ";
            if msg.starts_with(prefix) {
                let msg = msg.strip_prefix(prefix).expect("could not strip prefix");
                let cm = MessageType::ChatMessage(Connection::msg_to_chatmessage(msg.to_owned())?);

                return Ok(cm);
            }
            let prefix = "PRIVMSG ";
            if msg.starts_with(prefix) {
                let msg = msg.strip_prefix(prefix).expect("could not strip prefix");
                let cm =
                    MessageType::WhisperMessage(Connection::msg_to_whispermessage(msg.to_owned())?);
                return Ok(cm);
            }
        }
    }

    pub fn read_msg(&mut self) -> Result<ChatMessage, String> {
        let msg = self.read_until_prefix("MSG ")?;
        let v: Result<Value, _> = serde_json::from_str(&msg);
        match v {
            Ok(v) => {
                let message = v["data"].to_string();
                let sender = v["nick"].to_string();
                return Ok(ChatMessage {
                    message: clean_received(&message).to_owned(),
                    sender: clean_received(&sender).to_owned(),
                });
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    pub fn read_whisper(&mut self) -> Result<WhisperMessage, String> {
        let prefix = "PRIVMSG ";
        let msg = self.read_until_prefix(prefix)?;
        let v: Result<Value, _> = serde_json::from_str(&msg);
        match v {
            Ok(v) => {
                let message = v["data"].to_string();
                let sender = v["nick"].to_string();
                let receiver = v["nickTarget"].to_string();
                return Ok(WhisperMessage {
                    message: clean_received(&message).to_owned(),
                    sender: clean_received(&sender).to_owned(),
                    receiver: clean_received(&receiver).to_owned(),
                });
            }
            Err(e) => return Err(e.to_string()),
        }
    }

    pub fn read_any(&mut self) -> Result<String, String> {
        self.read()
    }
}

fn clean_received(s: &String) -> &str {
    let mut chars = s.chars();
    chars.next();
    chars.next_back();
    return chars.as_str();
}

#[cfg(test)]
mod tests {
    use crate::clean_received;

    #[test]
    fn test_clean_received() {
        let before = "\"abc\"".to_string();
        let cleaned = clean_received(&before);
        assert_eq!(cleaned, "abc");
    }
}
