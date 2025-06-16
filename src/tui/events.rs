use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, debug};

#[derive(Debug, Clone)]
pub enum Event {
    Key(KeyEvent),
    Tick,
    Error(String),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    _task: tokio::task::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        
        let _task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if tx.send(Event::Tick).is_err() {
                            debug!("Event channel closed, stopping tick handler");
                            break;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(1)) => {
                        // Check for key events frequently
                        if let Ok(true) = event::poll(Duration::from_millis(0)) {
                            match event::read() {
                                Ok(CrosstermEvent::Key(key)) => {
                                    if tx.send(Event::Key(key)).is_err() {
                                        debug!("Event channel closed, stopping input handler");
                                        break;
                                    }
                                }
                                Ok(_) => {
                                    // Ignore other event types
                                }
                                Err(e) => {
                                    error!("Failed to read terminal event: {}", e);
                                    let _ = tx.send(Event::Error(format!("Terminal read error: {}", e)));
                                }
                            }
                        }
                    }
                }
            }
            
            debug!("Event handler task ended");
        });
        
        Self { rx, _task }
    }
    
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}