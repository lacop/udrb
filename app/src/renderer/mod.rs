// use crate::chrome::ChromeDriver;
use crate::config::Config;
// use crate::slack;

use std::sync::mpsc;
use std::sync::Mutex;
// use std::thread;

// use log::{error, info};

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
    //     chrome: ChromeDriver,
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

// fn wrap_internal_error(e: failure::Error) -> RenderError {
//     RenderError::InternalError(e)
// }

// #[derive(Debug)]
// pub struct RenderResult {
//     // Title of the document.
//     pub title: String,
//     // URLs to the original document and rendered versions.
//     pub orig_url: String,
//     pub pdf_url: String,
//     pub png_url: Option<String>,
//     // User, channel and team names.
//     pub user: Option<String>,
//     pub channel: Option<String>,
//     pub team: Option<String>,
// }

// fn handle_request(
//     req: &RenderRequest,
//     config: &Config,
//     chrome: &mut ChromeDriver,
// ) -> Result<RenderResult, RenderError> {
//     if req.url.scheme() != "http" && req.url.scheme() != "https" {
//         return Err(RenderError::InvalidUrlError);
//     }
//     let host = req.url.domain().ok_or(RenderError::InvalidUrlError)?;
//     let mut domain_config = None;
//     for dc in &config.domains {
//         if dc.host_regex.is_match(host) {
//             domain_config = Some(dc);
//             break;
//         }
//     }
//     let domain_config = domain_config.ok_or(RenderError::UnsupportedDomain)?;

//     // Navigate to login page and run login script if specified.
//     if domain_config.login_page.is_some() {
//         chrome
//             .navigate(domain_config.login_page.as_ref().unwrap())
//             .map_err(wrap_internal_error)?;
//     }
//     if domain_config.login_script.is_some() {
//         chrome
//             .run_script(domain_config.login_script.as_ref().unwrap())
//             .map_err(wrap_internal_error)?;
//     }

//     // Navigate to the requested content.
//     chrome
//         .navigate(req.url.as_str())
//         .map_err(wrap_internal_error)?;

//     if domain_config.render_script.is_some() {
//         chrome
//             .run_script(domain_config.render_script.as_ref().unwrap())
//             .map_err(wrap_internal_error)?;
//     }

//     let title = chrome.get_title().map_err(wrap_internal_error)?;

//     // TODO use uri! macro with proper input.
//     let pdf_url = format!(
//         "{}/static/{}",
//         config.hostname,
//         chrome
//             .save_pdf(config.output_dir.as_path())
//             .map_err(wrap_internal_error)?
//     );

//     // TODO for now screenshot is optional and ignored when it fails
//     let screenshot_result = chrome
//         .save_screenshot(config.output_dir.as_path())
//         .map_err(wrap_internal_error);
//     if screenshot_result.is_err() {
//         error!("Screenshot failed: {:?}", screenshot_result);
//     }
//     let png_url = screenshot_result
//         .ok()
//         .map(|path| format!("{}/static/{}", config.hostname, path));

//     // TODO also do mhtml when content type is fixed

//     Ok(RenderResult {
//         title,
//         orig_url: req.url.as_str().to_string(),
//         pdf_url,
//         png_url,
//         user: req.user.as_ref().cloned(),
//         channel: req.channel.as_ref().cloned(),
//         team: req.team.as_ref().cloned(),
//     })
// }

impl Renderer {
    pub fn start(config: &Config) -> anyhow::Result<RenderSender> {
        // Render queue channel.
        let (sender, receiver) = mpsc::channel();

        // Initialize Chrome driver.
        //let chrome = ChromeDriver::new(&config.chrome_address)?;

        let mut renderer = Renderer {
            config: config.clone(),
            //chrome,
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
            //             let unknown = "?".to_string();
            //             info!(
            //                 "Handling request from @{} in #{} ({}): {:?}",
            //                 request.user.as_ref().unwrap_or(&unknown),
            //                 request.channel.as_ref().unwrap_or(&unknown),
            //                 request.team.as_ref().unwrap_or(&unknown),
            //                 request.url
            //             );
            //             let result = handle_request(&request, &self.config, &mut self.chrome);

            //             if request.slack_callback.is_some() {
            //                 let callback = request.slack_callback.as_ref().unwrap();
            //                 let slack_result = match result {
            //                     Ok(r) => {
            //                         info!("Request success: {:?}", r);
            //                         slack::post_success(callback, &r)
            //                     }
            //                     Err(e) => {
            //                         error!("Request failed: {:?}", e);
            //                         slack::post_failure(callback, &e)
            //                     }
            //                 };
            //                 if slack_result.is_err() {
            //                     error!("Slack posting failed: {:?}", slack_result.unwrap_err());
            //                 }
            //             } else {
            //                 info!("No slack callback, result: {:?}", result);
            //             }
        }
    }
}
