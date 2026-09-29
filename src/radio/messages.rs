use heapless::Vec;
use heapless::Deque;
use esp_hal::{time::{Instant, Duration}};

#[derive(Clone)]
pub enum Message {
    Packet(Vec<u8, 255>, i16, i16), // payload, rssi, snr
}


pub struct MessageQueue {
    horodatages: Deque<Instant, 256>,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self { horodatages: Deque::new() }
    }

    pub fn enregistrer(&mut self) {
        if self.horodatages.is_full() {
            self.horodatages.pop_front();
        }
        let _ = self.horodatages.push_back(Instant::now());
    }

    pub fn purger(&mut self) {
        while let Some(t) = self.horodatages.front() {
            if t.elapsed() > Duration::from_secs(60) {
                self.horodatages.pop_front();
            } else {
                break;  // les suivants sont plus récents
            }
        }
    }

    pub fn par_minute(&self) -> usize {
        self.horodatages.len()
    }
}