use crate::chrome::ChromeDriver;
use crate::config::Config;
use crate::slack;

use std::sync::mpsc;
use std::sync::Mutex;

use log::{error, info};

#[derive(Debug)]
pub struct RenderRequest {
    pub url: url::Url,
    pub slack_callback: String,
    pub user: Option<String>,
    pub channel: Option<String>,
    pub team: Option<String>,
}

// Send part of the render queue.
pub struct RenderSender(Mutex<mpsc::Sender<RenderRequest>>);

impl RenderSender {
    // Enqueues the request.
    pub fn render(&self, request: RenderRequest) -> anyhow::Result<()> {
        Ok(self.0.lock().unwrap().send(request)?)
    }
}

pub struct Renderer {
    config: Config,
    chrome: ChromeDriver,
    receiver: mpsc::Receiver<RenderRequest>,
}

#[derive(Debug)]
pub enum RenderError {
    InternalError(anyhow::Error),
    InvalidUrlError,
    UnsupportedDomain,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use RenderError::*;
        match self {
            InternalError(e) => write!(f, "Internal error ({:?})", e),
            InvalidUrlError => write!(f, "URL is not valid."),
            UnsupportedDomain => write!(f, "Domain is not supported."),
        }
    }
}

fn wrap_internal_error(e: anyhow::Error) -> RenderError {
    RenderError::InternalError(e)
}

#[derive(Debug)]
pub struct RenderResult {
    // Title of the document.
    pub title: String,
    // URLs to the original document and rendered versions.
    pub orig_url: String,
    pub pdf_url: String,
    pub png_url: Option<String>,
    // User, channel and team names.
    pub user: Option<String>,
    pub channel: Option<String>,
    pub team: Option<String>,
}

fn handle_request(
    req: &RenderRequest,
    config: &Config,
    chrome: &mut ChromeDriver,
) -> Result<RenderResult, RenderError> {
    if req.url.scheme() != "http" && req.url.scheme() != "https" {
        return Err(RenderError::InvalidUrlError);
    }
    let host = req.url.domain().ok_or(RenderError::InvalidUrlError)?;
    let domain_config = config
        .domains
        .iter()
        .find(|dc| dc.host.is_match(host))
        .ok_or(RenderError::UnsupportedDomain)?;

    // Navigate to login page and run login script if specified.
    if let Some(ref login_page) = domain_config.login_page {
        chrome.navigate(login_page).map_err(wrap_internal_error)?;
    }
    if let Some(ref login_script) = domain_config.login_script {
        chrome
            .run_script(login_script)
            .map_err(wrap_internal_error)?;
    }

    // Navigate to the requested content.
    chrome
        .navigate(req.url.as_str())
        .map_err(wrap_internal_error)?;

    if let Some(ref render_script) = domain_config.render_script {
        chrome
            .run_script(render_script)
            .map_err(wrap_internal_error)?;
    }

    let title = chrome.get_title().map_err(wrap_internal_error)?;

    // TODO use uri! macro with proper input.
    let to_url = |filename: &str| format!("{}/static/{}", config.hostname, filename);

    let pdf_file = chrome
        .save_pdf(config.output_dir.as_path())
        .map_err(wrap_internal_error)?;

    // TODO for now screenshot is optional and ignored when it fails
    let png_file = chrome
        .save_screenshot(config.output_dir.as_path())
        .map_err(wrap_internal_error);
    if png_file.is_err() {
        error!("Screenshot failed: {png_file:?}");
    }

    // TODO also do mhtml when content type is fixed
    Ok(RenderResult {
        title,
        orig_url: req.url.as_str().to_string(),
        pdf_url: to_url(&pdf_file),
        png_url: png_file.as_deref().map(to_url).ok(),
        user: req.user.clone(),
        channel: req.channel.clone(),
        team: req.team.clone(),
    })
}

impl Renderer {
    pub fn start(config: &Config) -> anyhow::Result<RenderSender> {
        // Render queue channel.
        let (sender, receiver) = mpsc::channel();

        // Initialize Chrome driver.
        let chrome = ChromeDriver::new(&config.chrome_address)?;

        let mut renderer = Renderer {
            config: config.clone(),
            chrome,
            receiver,
        };

        // Start render loop.
        std::thread::spawn(move || renderer.render_loop());

        // Return the sender for queueing RenderRequest.
        Ok(RenderSender(Mutex::new(sender)))
    }

    fn render_loop(&mut self) {
        for request in self.receiver.iter() {
            println!("{:?}", request);
            info!(
                "Handling request from @{} in #{} ({}): {:?}",
                request.user.as_deref().unwrap_or("?"),
                request.channel.as_deref().unwrap_or("?"),
                request.team.as_deref().unwrap_or("?"),
                request.url
            );
            let result = handle_request(&request, &self.config, &mut self.chrome);

            let slack_result = match result {
                Ok(result) => {
                    info!("Request success: {result:?}");
                    slack::post_success(&request.slack_callback, &result)
                }
                Err(err) => {
                    error!("Request failed: {err:?}");
                    slack::post_failure(&request.slack_callback, &err)
                }
            };
            if let Err(err) = slack_result {
                error!("Slack posting failed: {err:?}");
            }
        }
    }
}
