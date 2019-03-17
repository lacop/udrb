use crate::chrome::ChromeDriver;
use crate::config::Config;
use crate::slack;

use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;

#[derive(Debug)]
pub struct RenderRequest {
    pub url: url::Url,
    pub slack_callback: Option<String>,
}

// Send part of the render queue.
pub struct RenderSender(Mutex<mpsc::Sender<RenderRequest>>);

impl RenderSender {
    // Enqueues the request.
    pub fn render(&self, request: RenderRequest) -> Result<(), failure::Error> {
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
    InternalError(failure::Error),
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

fn wrap_internal_error(e: failure::Error) -> RenderError {
    RenderError::InternalError(e)
}

#[derive(Debug)]
pub struct RenderResult {
    // Title of the document.
    pub title: String,
    // URLs to the original document and rendered versions.
    pub orig_url: String,
    pub pdf_url: String,
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
        .get(host)
        .ok_or(RenderError::UnsupportedDomain)?;
    println!("{:?} {:?} {:?}", req, host, domain_config);

    // Navigate to login page and run login script if specified.
    if domain_config.login_page.is_some() {
        chrome
            .navigate(domain_config.login_page.as_ref().unwrap())
            .map_err(wrap_internal_error)?;
    }
    if domain_config.login_script.is_some() {
        chrome
            .run_script(domain_config.login_script.as_ref().unwrap())
            .map_err(wrap_internal_error)?;
    }

    // Navigate to the requested content.
    chrome
        .navigate(req.url.as_str())
        .map_err(wrap_internal_error)?;

    let title = chrome.get_title().map_err(wrap_internal_error)?;

    // TODO also do screenshot when fixed
    // TODO also do mhtml when content type is fixed
    let pdf_path = chrome
        .save_pdf(config.output_dir.as_path())
        .map_err(wrap_internal_error)?;

    // TODO use uri! macro with proper input.
    Ok(RenderResult {
        title: title,
        orig_url: req.url.as_str().to_string(),
        pdf_url: format!("{}/static/{}", config.hostname, pdf_path),
    })
}

impl Renderer {
    pub fn start(config: &Config) -> Result<RenderSender, failure::Error> {
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
        thread::spawn(move || renderer.render_loop());

        // Return the sender for queueing RenderRequest.
        Ok(RenderSender(Mutex::new(sender)))
    }

    fn render_loop(&mut self) {
        for request in self.receiver.iter() {
            println!("Handling request {:?}", request);
            let result = handle_request(&request, &self.config, &mut self.chrome);
            println!("Request result: {:?}", result);

            if request.slack_callback.is_some() {
                let callback = request.slack_callback.as_ref().unwrap();
                let slack_result = match result {
                    Ok(r) => slack::post_success(callback, &r),
                    Err(e) => slack::post_failure(callback, &e),
                };
                println!("Slack posting result: {:?}", slack_result);
            }
        }
    }
}
