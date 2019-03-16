extern crate base64;
extern crate crypto;
extern crate reqwest;
extern crate websocket;

use crypto::digest::Digest;
use crypto::sha3::Sha3;

use std::fs::File;
use std::io::Write;

use websocket::client::sync::Client;
use websocket::stream::sync::TcpStream;

pub struct ChromeDriver {
    ws: Client<TcpStream>,
    message_id: u32,
}

#[derive(Serialize)]
struct ChromeCommandRequest {
    id: u32,
    method: String,
    params: serde_json::Value,
}

fn write_bytes_to_directory(bytes: &[u8], dir: &std::path::Path, suffix: &str) -> Result<std::path::PathBuf, failure::Error> {
    let mut hasher = Sha3::sha3_256();
    hasher.input(&bytes);
    
    let filename = hasher.result_str() + suffix;
    let output_path = dir.join(filename);

    std::fs::create_dir_all(dir)?;
    let mut buffer = File::create(&output_path)?;
    buffer.write(&bytes)?;
    
    Ok(output_path)
}

fn write_base64_to_directory(data: &str, dir: &std::path::Path, suffix: &str) -> Result<std::path::PathBuf, failure::Error> {
    let bytes = base64::decode(data)?;
    write_bytes_to_directory(&bytes, dir, suffix)
}

fn write_text_to_directory(data: &str, dir: &std::path::Path, suffix: &str) -> Result<std::path::PathBuf, failure::Error> {
    write_bytes_to_directory(data.as_bytes(), dir, suffix)
}

impl ChromeDriver {
    pub fn new() -> Result<ChromeDriver, failure::Error> {
        let body = reqwest::get("http://127.0.0.1:9222/json/list")?.text()?;
        let body: serde_json::Value = serde_json::from_str(&body)?;
        let list = body.as_array().ok_or(format_err!("Expected array"))?;
        ensure!(list.len() > 0, "Need at least one existing tab");

        let websocket_url = list[0]["webSocketDebuggerUrl"]
            .as_str()
            .ok_or(format_err!("Invalid websocket url"))?;
        let ws = websocket::ClientBuilder::new(&websocket_url)?.connect_insecure()?;

        let mut chrome =  ChromeDriver {
            ws: ws,
            message_id: 0,
        };
        
        // TODO proper await for events
        // chrome.chrome_command("Page.setLifecycleEventsEnabled", json!({"enabled": false}))?;

        Ok(chrome)
    }

    fn send_command(&mut self, method: &str, params: serde_json::Value) -> Result<u32, failure::Error> {
        let command = ChromeCommandRequest {
            id: self.message_id,
            method: method.to_string(),
            params: params,
        };
        self.message_id += 1;

        let message = websocket::Message::text(serde_json::to_string(&command)?);
        self.ws.send_message(&message).map_err(|_| format_err!("Failed to send"))?;
        Ok(command.id)
    }

    fn get_result(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, failure::Error> {
        let id = self.send_command(method, params)?;
        loop {
            match self.ws.recv_message()? {
                websocket::OwnedMessage::Text(response) => {
                    let response : serde_json::Value = serde_json::from_str(&response)?;
                    if response["id"] != id {
                        continue;
                    }
                    // TODO avoid clone, move out of borrowed should be fine here
                    return Ok(response["result"].clone());
                },
                _ => {
                    return Err(format_err!("Unexpected return message type"));
                }
            }
        }
    }

    pub fn navigate(&mut self, url: &str) -> Result<(), failure::Error> {
        self.send_command("Page.navigate", json!({"url": url}))?;
        // TODO proper wait
        std::thread::sleep(std::time::Duration::from_secs(2));        
        Ok(())
    }

    pub fn save_screenshot(&mut self, dir: &std::path::Path) -> Result<std::path::PathBuf, failure::Error> {
        let result = self.get_result("Page.getLayoutMetrics", serde_json::Value::Null)?;
        let width = result["contentSize"]["width"].as_i64().ok_or(format_err!("Missing dimension"))?;
        let height = result["contentSize"]["height"].as_i64().ok_or(format_err!("Missing dimension"))?;
        
        let params = json!({"clip": {"x": 0, "y": 0, "width": width, "height": height, "scale": 1}});
        let result = self.get_result("Page.captureScreenshot", params)?;
        let data = result["data"].as_str().ok_or(format_err!("Missing data"))?;
        write_base64_to_directory(data, dir, ".png")
    }

    pub fn save_pdf(&mut self, dir: &std::path::Path) -> Result<std::path::PathBuf, failure::Error> {
        // A4 paper size in inches.
        let params = json!({"landscape": false, "scale": 1, "paperWidth": 8.27, "paperHeight": 11.69});
        let result = self.get_result("Page.printToPDF", params)?;
        let data = result["data"].as_str().ok_or(format_err!("Missing data"))?;
        write_base64_to_directory(data, dir, ".pdf")
    }

    pub fn save_mhtml(&mut self, dir: &std::path::Path) -> Result<std::path::PathBuf, failure::Error> {
        let result = self.get_result("Page.captureSnapshot", serde_json::Value::Null)?;
        let data = result["data"].as_str().ok_or(format_err!("Missing data"))?;
        write_text_to_directory(data, dir, ".mhtml")
    }
}