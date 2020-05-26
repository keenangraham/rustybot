use std::thread::JoinHandle;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;


pub type Worker = (String, JoinHandle<()>, Arc<AtomicBool>, String);

pub type Workers = Vec<Worker>;

pub const BOT_ID: &str = "<@U013X667NR4>";

pub const PROD_INDEXER_URL: &str = "https://www.encodeproject.org/_indexer";

pub const EMOJIS: [&str; 7] = [
    ":hugging_face:",
    ":lion_face:",
    ":see_no_evil:",
    ":duck:",
    ":palm_tree:",
    ":microscope:",
    ":man-surfing:"
];
