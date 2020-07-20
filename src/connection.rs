use crate::bot;
use crate::constants::{self, Worker, Workers};
use slack::{self, Event, RtmClient, Message};
use std::thread;
use slack_api::{self, MessageStandard};
use crossbeam_channel::{unbounded, Sender, Receiver};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::sync::Arc;
use regex::Regex;
use lazy_static;


fn get_worker_id() -> usize {
    static COUNTER:AtomicUsize = AtomicUsize::new(1000);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}


pub struct Connection {
    token: String,
    tx: Sender<String>,
    rx: Receiver<String>,
    workers: Workers
}


impl Connection {

    pub fn new(token: &str) -> Self {
	let (tx, rx) = unbounded();
	Connection {
	    token: token.to_string(),
	    tx: tx,
	    rx: rx,
	    workers: vec![]
	}
    }
    
    pub fn listen(&mut self,) -> Result<(), slack::error::Error> {
	let mut count = 0;
	loop {
	    println!("LOOP {}", count);
            let rtm = RtmClient::login_and_run(&self.token.to_owned(), self);
	    println!("{:?}", rtm);
	    count += 1;
	}
    }

    fn maybe_get_message_from_event<'a>(&self, event: &'a Event) -> Option<&'a Message> {
        match event {
	    Event::Message(message) => Some(&message),
     	    _ => None
        }
    }

    fn get_new_worker_id_and_clone(&self) -> (String, String) {
	let worker_id = get_worker_id().to_string();
	let worker_id_clone = worker_id.clone();
	(worker_id, worker_id_clone)
    }

    fn register_bot(&mut self, worker: Worker) {
	self.workers.push(worker);
    }

    fn remove_multiple_spaces(&self, message_text: String) -> String {
	lazy_static! {
            static ref MULTIPLE_SPACES: Regex = Regex::new(
		r"\s\s+"
            ).unwrap();
	}
	MULTIPLE_SPACES.replace_all(&message_text, " ").to_string()
    }

    fn remove_nonbreaking_space(&self, message_text: String) -> String {
	message_text.replace("\u{a0}", " ").to_string()
    }

    fn clean_message_text(&self, message_text: String) -> String {
	self.remove_multiple_spaces(
	    self.remove_nonbreaking_space(message_text)
	)
    }

    fn clean_slack_message(&self, message: &MessageStandard) -> MessageStandard {
	MessageStandard{
	    text: Some(
		self.clean_message_text(
		    message.text.as_ref().unwrap().to_string()
		)
	    ),
	    ..message.clone()
	}
    }

    fn spawn_thread(&mut self, message: slack_api::MessageStandard) {
	let message_text = message.text.as_ref().unwrap().to_string();
        let (worker_id, worker_id_clone) = self.get_new_worker_id_and_clone();
	let is_cancelled = Arc::new(AtomicBool::new(false));
   	let rustybot = bot::RustyBot::new(
	    self.token.clone(),
	    worker_id_clone,
	    self.tx.clone(),
	    is_cancelled.clone(),
	);
        let handle = thread::spawn(
	    move || {
		rustybot.handle_message(message);
	    }
	);
	self.register_bot((worker_id, handle, is_cancelled, message_text));
    }

    fn cancel_bot_by_worker_id(&mut self, worker_id: &String, channel: &Option<String>, cli: &RtmClient) {
	for bot in self.workers.iter() {
    	    if &bot.0 == worker_id {
		cli.sender().send_message(
		    &channel.as_ref().unwrap(),
		    &format!("Canceling {}", worker_id)
		);
	        bot.2.store(true, Ordering::Relaxed);
		return;
	    }
	}
	cli.sender().send_message(
	    &channel.as_ref().unwrap(),
	    &format!("No active job {} found", worker_id)
	);
    }

    fn pop_bot_by_worker_id(&mut self, worker_id: &String) -> Option<Worker> {
	let mut index: Option<usize> = None;
	for (i, bot) in self.workers.iter().enumerate() {
	    if &bot.0 == worker_id {
		index = Some(i);
	    }
	}
        if let Some(index) = index {
	    return Some(self.workers.swap_remove(index));
	}
	None
    }

    fn join_completed_threads(&mut self) {
	let worker_ids: Vec<String> = self.rx.try_iter().collect();
        for worker_id in worker_ids {
	    if let Some(bot) = self.pop_bot_by_worker_id(&worker_id) {
		println!("Joining {:?}", &bot.0);
	        bot.1.join().unwrap_or_else(
		    |x| println!("Error joining")
		);
	    }
	};
    }
    
    fn should_cancel_job(&self, text: &Option<String>) -> Option<String> {
	lazy_static! {
            static ref JOB_RE: Regex = Regex::new(r"(cancel|stop) (\d+)").unwrap();
	}
	if let Some(message) = text {
	    if message.starts_with(constants::BOT_ID) {
		if let Some(capture) = JOB_RE.captures(message) {
		    return Some(capture.get(2).unwrap().as_str().to_owned());
		}
	    }
	}
	None
    }

    fn should_list_active_jobs(&self, text: &Option<String>) -> bool {
	if let Some(message) = text {
	    if message.starts_with(constants::BOT_ID) && message.contains("list") {
		return true
	    }
	}
	false
    }

    fn list_jobs(&mut self, channel: &Option<String>, cli: &RtmClient) {
	let jobs: Vec<(String, String)> = self.workers.iter().map(
	    |bot| {
		(bot.0.to_owned(), bot.3.to_owned())
	    }
	).collect();
	cli.sender().send_message(&channel.as_ref().unwrap(), &format!("{:?}", jobs));
    }

    fn should_pass_message_to_bot(&self, text: &Option<String>) -> bool {
	if let Some(message) = text {
	    if message.starts_with(constants::BOT_ID) {
		return true
	    }
	}
	false
    }

    fn handle_message(&mut self, cli: &RtmClient, message: &MessageStandard) {
	if let Some(worker_id) = self.should_cancel_job(&message.text) {
	    self.cancel_bot_by_worker_id(&worker_id, &message.channel, cli);
	} else if self.should_list_active_jobs(&message.text) {
	    self.list_jobs(&message.channel, cli);
	} else if self.should_pass_message_to_bot(&message.text) {
	    self.spawn_thread(message.to_owned());
	}
    }
}


impl slack::EventHandler for Connection {
    fn on_event(&mut self, cli: &RtmClient, event: Event) {
        let maybe_message = self.maybe_get_message_from_event(&event);
	match maybe_message {
	    Some(Message::Standard(message)) => {
		self.handle_message(cli, &self.clean_slack_message(message));
	    },
	    _ => {}
	}
	self.join_completed_threads();
    }

    fn on_close(&mut self, cli: &RtmClient) {
        println!("Closing!");
    }

    fn on_connect(&mut self, cli: &RtmClient) {
        println!("Connected!");
    }
}
