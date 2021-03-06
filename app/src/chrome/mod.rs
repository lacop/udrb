extern crate base64;
extern crate crypto;
extern crate dns_lookup;
extern crate reqwest;
extern crate websocket;

use crypto::digest::Digest;
use crypto::sha3::Sha3;

use std::fs::File;
use std::io::Write;

use websocket::client::sync::Client;
use websocket::stream::sync::TcpStream;

use log::info;

pub struct ChromeDriver {
    address: String,
    ws: Option<Client<TcpStream>>,
    message_id: u32,
}

#[derive(Serialize)]
struct ChromeCommandRequest {
    id: u32,
    method: String,
    params: serde_json::Value,
}

fn write_bytes_to_directory(
    bytes: &[u8],
    dir: &std::path::Path,
    suffix: &str,
) -> Result<String, failure::Error> {
    let mut hasher = Sha3::sha3_256();
    hasher.input(&bytes);

    let filename = hasher.result_str() + suffix;
    let output_path = dir.join(&filename);

    std::fs::create_dir_all(dir)?;
    let mut buffer = File::create(&output_path)?;
    buffer.write_all(&bytes)?;

    Ok(filename)
}

fn write_base64_to_directory(
    data: &str,
    dir: &std::path::Path,
    suffix: &str,
) -> Result<String, failure::Error> {
    let bytes = base64::decode(data)?;
    write_bytes_to_directory(&bytes, dir, suffix)
}

fn write_text_to_directory(
    data: &str,
    dir: &std::path::Path,
    suffix: &str,
) -> Result<String, failure::Error> {
    write_bytes_to_directory(data.as_bytes(), dir, suffix)
}

impl ChromeDriver {
    pub fn new(address: &str) -> Result<ChromeDriver, failure::Error> {
        let chrome = ChromeDriver {
            address: address.to_string(),
            ws: None,
            message_id: 0,
        };
        // TODO proper await for events
        // chrome.chrome_command("Page.setLifecycleEventsEnabled", json!({"enabled": false}))?;
        Ok(chrome)
    }

    // Check if we can talk to chrome and try to reconnect if not.
    // If chrome crashes in the background (sometimes happens for reason we don't understand) Docker will restart it,
    // but the socket will be lost and we need to redo the connection.
    fn maybe_connect(&mut self) -> Result<(), failure::Error> {
        if self.ws.is_some() {
            // If we have socket try to send arbitrary command to verify it is still alive.
            let command = ChromeCommandRequest {
                id: self.message_id,
                method: "Browser.getVersion".to_string(),
                params: serde_json::Value::Null,
            };
            self.message_id += 1;
            let message = websocket::Message::text(serde_json::to_string(&command)?);
            let send_result = self
                .ws
                .as_mut()
                .ok_or(format_err!("Lost socket"))?
                .send_message(&message);
            if send_result.is_ok() {
                // We managed to send something. Ignore the reply, just return as we have a valid connection.
                return Ok(());
            }
        }
        // If we have no socket or sending failed we need to establish new connection.
        info!("Restarting chrome connection...");

        // Chrome only allows connection when the host header is either
        // localhost or IP, so the "chrome:port" value from docker compose
        // wouldn't work. Resolve to IP manually.
        let address: Vec<_> = self.address.split(':').collect();
        let (hostname, port) = (address[0], address[1]);
        let ips = dns_lookup::lookup_host(hostname)?;
        let ip = ips.first().ok_or_else(|| format_err!("Lookup failed"))?;

        let json_url = format!("http://{}:{}/json/list", ip, port);
        let body = reqwest::get(json_url.as_str())?.text()?;
        let body: serde_json::Value = serde_json::from_str(&body)?;
        let list = body
            .as_array()
            .ok_or_else(|| format_err!("Expected array"))?;
        ensure!(!list.is_empty(), "Need at least one existing tab");

        let websocket_url = list[0]["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| format_err!("Invalid websocket url"))?;
        self.ws = Some(websocket::ClientBuilder::new(&websocket_url)?.connect_insecure()?);
        Ok(())
    }

    fn send_command(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<u32, failure::Error> {
        let command = ChromeCommandRequest {
            id: self.message_id,
            method: method.to_string(),
            params: params,
        };
        self.message_id += 1;
        let message = websocket::Message::text(serde_json::to_string(&command)?);

        // Check connection before sending to recover from previous crashes.
        self.maybe_connect()?;
        self.ws
            .as_mut()
            .ok_or(format_err!("Lost socket"))?
            .send_message(&message)
            .map_err(|_| format_err!("Failed to send"))?;
        Ok(command.id)
    }

    fn get_result(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, failure::Error> {
        let id = self.send_command(method, params)?;
        loop {
            // If send_command was successful we should have a valid socket around.
            match self
                .ws
                .as_mut()
                .ok_or(format_err!("Lost socket"))?
                .recv_message()?
            {
                websocket::OwnedMessage::Text(response) => {
                    let response: serde_json::Value = serde_json::from_str(&response)?;
                    if response["id"] != id {
                        continue;
                    }
                    // TODO avoid clone, move out of borrowed should be fine here
                    return Ok(response["result"].clone());
                }
                _ => {
                    return Err(format_err!("Unexpected return message type"));
                }
            }
        }
    }

    pub fn navigate(&mut self, url: &str) -> Result<(), failure::Error> {
        self.send_command("Page.navigate", json!({ "url": url }))?;
        // TODO proper wait
        std::thread::sleep(std::time::Duration::from_secs(5));
        Ok(())
    }

    // TODO try to safeguard against too big pages with some hard limits
    pub fn save_screenshot(&mut self, dir: &std::path::Path) -> Result<String, failure::Error> {
        let result = self.get_result("Page.getLayoutMetrics", serde_json::Value::Null)?;
        let width = result["contentSize"]["width"]
            .as_i64()
            .ok_or_else(|| format_err!("Missing dimension"))?;
        let height = result["contentSize"]["height"]
            .as_i64()
            .ok_or_else(|| format_err!("Missing dimension"))?;

        let params = json!({"width": width, "screenWidth": width,
                            "height": height, "screenHeight": height,
                            "scale": 1, "deviceScaleFactor": 1,
                            "mobile": false});
        let _ = self.get_result("Emulation.setDeviceMetricsOverride", params)?;

        let params =
            json!({"clip": {"x": 0, "y": 0, "width": width, "height": height, "scale": 1}});
        let result = self.get_result("Page.captureScreenshot", params)?;
        let data = result["data"]
            .as_str()
            .ok_or_else(|| format_err!("Missing data"))?;
        write_base64_to_directory(data, dir, ".png")
    }

    pub fn save_pdf(&mut self, dir: &std::path::Path) -> Result<String, failure::Error> {
        // A4 paper size in inches.
        let params =
            json!({"landscape": false, "scale": 1, "paperWidth": 8.27, "paperHeight": 11.69});
        let result = self.get_result("Page.printToPDF", params)?;
        let data = result["data"]
            .as_str()
            .ok_or_else(|| format_err!("Missing data"))?;
        write_base64_to_directory(data, dir, ".pdf")
    }

    pub fn save_mhtml(&mut self, dir: &std::path::Path) -> Result<String, failure::Error> {
        let result = self.get_result("Page.captureSnapshot", serde_json::Value::Null)?;
        let data = result["data"]
            .as_str()
            .ok_or_else(|| format_err!("Missing data"))?;
        write_text_to_directory(data, dir, ".mhtml")
    }

    pub fn run_script(&mut self, script: &str) -> Result<(), failure::Error> {
        let params = json!({"expression": script, "returnByValue": false});
        let _result = self.get_result("Runtime.evaluate", params)?;
        // TODO avoid sleep by handling the result somehow?
        std::thread::sleep(std::time::Duration::from_secs(3));
        Ok(())
    }

    pub fn get_title(&mut self) -> Result<String, failure::Error> {
        let params = json!({"expression": "document.title", "returnByValue": true});
        let result = self.get_result("Runtime.evaluate", params)?;
        let title = result["result"]["value"]
            .as_str()
            .ok_or_else(|| format_err!("Failed to get title"))?;
        Ok(title.to_string())
    }
}
