//pub mod chrome;
use crate::chrome::ChromeDriver;


use std::thread;
use std::sync::mpsc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct RenderRequest {
    pub url: String,
    pub slack_callback: Option<String>
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
    // TODO config
    receiver: mpsc::Receiver<RenderRequest>,
    chrome: ChromeDriver,
}

impl Renderer {
    pub fn start() -> Result<RenderSender, failure::Error> {
        // Render queue channel.
        let (sender, receiver) = mpsc::channel();
        // Initialize Chrome driver.
        let chrome = ChromeDriver::new()?;
        // TODO parse config
        
        let mut renderer = Renderer {receiver, chrome};

        // Start render loop.
        thread::spawn(move || {renderer.render_loop()});
        
        // Return the sender for queueing RenderRequest.
        Ok(RenderSender(Mutex::new(sender)))
    }

    fn render_loop(&mut self) -> () {
        for request in self.receiver.iter() {
            println!("Handling request {:?}", request);
            //self.chrome.navigate(request.url)?
            println!("Done {:?}", request);
        }
    }
}
