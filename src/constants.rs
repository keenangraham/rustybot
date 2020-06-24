use std::thread::JoinHandle;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;


pub type Worker = (String, JoinHandle<()>, Arc<AtomicBool>, String);

pub type Workers = Vec<Worker>;

pub const BOT_ID: &str = "<@U013X667NR4>";

pub const EMOJIS: [&str; 7] = [
    ":hugging_face:",
    ":lion_face:",
    ":see_no_evil:",
    ":duck:",
    ":palm_tree:",
    ":microscope:",
    ":man-surfing:"
];

pub const HELP: &str = r#"
USAGE:
    <@rustybot> [COMMAND]
COMMANDS:
    *help*     Print this message
    *list*     List active jobs
    *stop/cancel [JOB_ID]*     Cancel job
    *monitor [URL]*     Monitor URL until indexing is complete
    *status [URL]*     Return URL indexer status and results
    *ec2 info [URL]*    Get instance info
    *ec2 start [URL]*    Start instance
    *ec2 stop [URL]*    Stop instance
    *ec2 resize [URL] --size [SIZE]*     Resize instance (default r5.2xlarge)
"#;

pub const RESIZE_INSTANCE: &str = "r5.2xlarge";
