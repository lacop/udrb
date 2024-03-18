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
    log::warn!("Internal error: {:?}", e);
    RenderError::InternalError(e)
}

#[derive(Debug)]
pub struct RenderResult {
    // Title of the document.
    pub title: String,
    // URLs to the original document and rendered versions.
    pub orig_url: url::Url,
    pub pdf_url: Option<String>,
    pub png_url: Option<String>,
    pub mhtml_url: Option<String>,
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

    // All these are optional and ignored when they fail.
    let to_url = |filename: &str| format!("{}/static/{}", config.hostname, filename);
    let pdf_file = chrome
        .save_pdf(config.output_dir.as_path())
        .map_err(wrap_internal_error);
    let png_file = chrome
        .save_screenshot(config.output_dir.as_path())
        .map_err(wrap_internal_error);
    let mhtml_file = chrome
        .save_mhtml(config.output_dir.as_path())
        .map_err(wrap_internal_error);

    // Require that at least PDF of PNG is available (MHTML is experimental, it alone
    // is not enough to consider this a success).
    if pdf_file.is_err() && png_file.is_err() {
        return Err(RenderError::InternalError(anyhow::anyhow!(
            "Failed to capture either PDF or screenshot"
        )));
    }

    Ok(RenderResult {
        title,
        orig_url: req.url.clone(),
        pdf_url: pdf_file.as_deref().map(to_url).ok(),
        png_url: png_file.as_deref().map(to_url).ok(),
        mhtml_url: mhtml_file.as_deref().map(to_url).ok(),
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
