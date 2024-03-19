mod info;
pub use info::PageInfo;

use std::fs::File;
use std::io::Write;

use anyhow::format_err;
use base64::Engine;
use log::{info, warn};
use serde_json::json;
use sha3::Digest;
use websocket::client::sync::Client;
use websocket::stream::sync::TcpStream;

pub struct ChromeDriver {
    address: String,
    kill_address: String,
    ws: Option<Client<TcpStream>>,
    message_id: u32,
}

#[derive(serde::Serialize)]
struct ChromeCommandRequest {
    id: u32,
    method: String,
    params: serde_json::Value,
}

fn bytes_to_hash(bytes: &[u8]) -> String {
    let mut hasher = sha3::Sha3_256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn write_base64_to_directory(
    data: &str,
    dir: &std::path::Path,
    suffix: &str,
) -> anyhow::Result<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    let filename = bytes_to_hash(&bytes) + suffix;
    let output_path = dir.join(&filename);

    std::fs::create_dir_all(dir)?;
    let mut buffer = File::create(output_path)?;
    buffer.write_all(&bytes)?;

    Ok(filename)
}

fn get_extension(headers: &[mail_parser::Header]) -> Option<&'static str> {
    for header in headers {
        if header.name != mail_parser::HeaderName::ContentType {
            continue;
        }
        let content_type = match &header.value {
            mail_parser::HeaderValue::ContentType(ct) => ct,
            _ => {
                warn!("Invalid content type {:?}", header);
                continue;
            }
        };
        match (content_type.ctype(), content_type.subtype()) {
            ("text", Some("html")) => return Some("html"),
            ("text", Some("css")) => return Some("css"),
            ("text", Some("javascript")) => return Some("js"),
            ("image", Some("jpeg")) => return Some("jpg"),
            ("image", Some("png")) => return Some("png"),
            ("image", Some("gif")) => return Some("gif"),
            ("image", Some("bmp")) => return Some("bmp"),
            ("image", Some("svg+xml")) => return Some("svg"),
            ("image", Some("webp")) => return Some("webp"),
            ("application", Some("pdf")) => return Some("pdf"),
            _ => {
                warn!("Unknown content type {:?}", content_type);
                return None;
            }
        }
    }
    None
}

fn get_content_location(headers: &[mail_parser::Header]) -> Option<String> {
    // Content-ID first as Chrome has replaced the href/src with "cid:...".
    for header in headers {
        if header.name != mail_parser::HeaderName::ContentId {
            continue;
        }
        return match &header.value {
            mail_parser::HeaderValue::Text(t) => {
                let t = t.trim_start_matches('<').trim_end_matches('>');
                Some(format!("cid:{}", t))
            }
            _ => {
                warn!("Invalid content id {:?}", header);
                None
            }
        };
    }
    // Fall back to Content-Location if Content-ID is missing.
    for header in headers {
        if header.name != mail_parser::HeaderName::ContentLocation {
            continue;
        }
        return match &header.value {
            mail_parser::HeaderValue::Text(t) => Some(t.to_string()),
            _ => {
                warn!("Invalid content location {:?}", header);
                None
            }
        };
    }
    None
}

fn write_mhtml_to_directory(
    data: &str,
    dir: &std::path::Path,
) -> anyhow::Result<(String, Option<PageInfo>)> {
    let message = mail_parser::MessageParser::default()
        .parse(data.as_bytes())
        .ok_or(format_err!("Failed to parse mhtml"))?;

    // At least two parts: First is the header, second is the downloaded page itself.
    anyhow::ensure!(message.parts.len() >= 2, "Too few parts in mhtml");
    anyhow::ensure!(
        matches!(message.parts[0].body, mail_parser::PartType::Multipart(_)),
        "First part is not multipart"
    );

    // Ensure output directory exists.
    let hash = bytes_to_hash(data.as_bytes());
    let dir = dir.join(&hash);
    std::fs::create_dir_all(&dir)?;

    // Dump the raw MHTML for potential debugging.
    {
        let mut file = File::create(dir.join("raw.mhtml"))?;
        file.write_all(data.as_bytes())?;
        info!("Wrote {}/raw.mhtml", hash);
    }

    // Write out all the parts except the first one, and remember the filenames.
    let mut part_filenames = std::collections::HashMap::new();
    for part in &message.parts[2..] {
        let filename = format!(
            "{}.{}",
            part_filenames.len() + 1,
            get_extension(&part.headers).ok_or(anyhow::format_err!("Unknown content type"))?
        );
        let path = dir.join(&filename);
        let content_location = match get_content_location(&part.headers) {
            Some(cl) => cl,
            None => {
                // Some stuff  might be missing Content-Location, ignore it.
                println!("no content_location {:?}", part.headers);
                continue;
            }
        };
        part_filenames.insert(content_location, filename);

        let mut file = File::create(path)?;
        match &part.body {
            mail_parser::PartType::Text(text) => file.write_all(text.as_bytes())?,
            mail_parser::PartType::Html(text) => file.write_all(text.as_bytes())?,
            mail_parser::PartType::Binary(data) => file.write_all(data)?,
            _ => return Err(format_err!("Unexpected body")),
        }
    }

    // Write out the index.html file with the correct references to the other files.
    let index_path = dir.join("index.html");
    let mut index_file = File::create(index_path)?;
    let page_info;
    if let mail_parser::PartType::Html(html) = &message.parts[1].body {
        let mut html = html.to_string();
        for (content_location, filename) in part_filenames {
            html = html.replace(&content_location, &filename);
        }
        index_file.write_all(html.as_bytes())?;
        page_info = PageInfo::from_html(&html).ok();
    } else {
        return Err(format_err!("Unexpected body for index"));
    }

    Ok((format!("{}/index.html", hash), page_info))
}

// TODO: Rewrite in a way that is robust to Chrome hanging or dieing. Something like:
// - Have timeouts for websocket reads/writes.
// - Run each render request in own thread which borrows the chrome connection.
// - If the whole operation times out cancel the thread, any attempt to access
//   the chrome connection from it should return error and let the thread die.
impl ChromeDriver {
    pub fn new(address: &str, kill_address: &str) -> anyhow::Result<ChromeDriver> {
        let chrome = ChromeDriver {
            address: address.to_string(),
            kill_address: kill_address.to_string(),
            ws: None,
            message_id: 0,
        };
        // Connect to return error early if misconfigured.
        // TODO: This can fail because chrome might not be ready yet on "docker compose up".
        //       Retry a few times, or something?
        // chrome.maybe_connect()?;

        // TODO: Proper await for events via Page.setLifecycleEventsEnabled ?
        Ok(chrome)
    }

    pub fn kill(&self) -> anyhow::Result<()> {
        let response = reqwest::blocking::get(&self.kill_address)?;
        if !response.status().is_success() {
            return Err(format_err!("Failed to kill chrome"));
        }
        Ok(())
    }

    // Check if we can talk to chrome and try to reconnect if not.
    // If chrome crashes in the background (sometimes happens for reason we don't understand) Docker will restart it,
    // but the socket will be lost and we need to redo the connection.
    fn maybe_connect(&mut self) -> anyhow::Result<()> {
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
        let body = reqwest::blocking::get(json_url.as_str())?.text()?;
        let body: serde_json::Value = serde_json::from_str(&body)?;
        let list = body
            .as_array()
            .ok_or_else(|| format_err!("Expected array"))?;
        anyhow::ensure!(!list.is_empty(), "Need at least one existing tab");

        let websocket_url = list[0]["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| format_err!("Invalid websocket url"))?;
        self.ws = Some(websocket::ClientBuilder::new(websocket_url)?.connect_insecure()?);
        Ok(())
    }

    fn send_command(&mut self, method: &str, params: serde_json::Value) -> anyhow::Result<u32> {
        let command = ChromeCommandRequest {
            id: self.message_id,
            method: method.to_string(),
            params,
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
    ) -> anyhow::Result<serde_json::Value> {
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
                    let mut response: serde_json::Value = serde_json::from_str(&response)?;
                    if response["id"] != id {
                        continue;
                    }
                    return Ok(response
                        .get_mut("result")
                        .ok_or(anyhow::format_err!("Missing result"))?
                        .take());
                }
                _ => {
                    return Err(format_err!("Unexpected return message type"));
                }
            }
        }
    }

    pub fn navigate(&mut self, url: &str) -> anyhow::Result<()> {
        self.send_command("Page.navigate", json!({ "url": url }))?;
        // TODO: Proper wait.
        std::thread::sleep(std::time::Duration::from_secs(5));
        Ok(())
    }

    // TODO: Try to safeguard against too big pages with some hard limits.
    pub fn save_screenshot(&mut self, dir: &std::path::Path) -> anyhow::Result<String> {
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

    pub fn save_pdf(&mut self, dir: &std::path::Path) -> anyhow::Result<String> {
        // A4 paper size in inches.
        let params =
            json!({"landscape": false, "scale": 1, "paperWidth": 8.27, "paperHeight": 11.69});
        let result = self.get_result("Page.printToPDF", params)?;
        let data = result["data"]
            .as_str()
            .ok_or_else(|| format_err!("Missing data"))?;
        write_base64_to_directory(data, dir, ".pdf")
    }

    pub fn save_mhtml(
        &mut self,
        dir: &std::path::Path,
    ) -> anyhow::Result<(String, Option<PageInfo>)> {
        let result = self.get_result("Page.captureSnapshot", serde_json::Value::Null)?;
        let data = result["data"]
            .as_str()
            .ok_or_else(|| format_err!("Missing data"))?;
        write_mhtml_to_directory(data, dir)
    }

    pub fn run_script(&mut self, script: &str) -> anyhow::Result<()> {
        let params = json!({"expression": script, "returnByValue": false});
        let _result = self.get_result("Runtime.evaluate", params)?;
        // TODO avoid sleep by handling the result somehow?
        std::thread::sleep(std::time::Duration::from_secs(3));
        Ok(())
    }

    pub fn get_title(&mut self) -> anyhow::Result<String> {
        let params = json!({"expression": "document.title", "returnByValue": true});
        let result = self.get_result("Runtime.evaluate", params)?;
        let title = result["result"]["value"]
            .as_str()
            .ok_or_else(|| format_err!("Failed to get title"))?;
        Ok(title.to_string())
    }
}
