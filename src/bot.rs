use crate::constants;
use serde::Deserialize;
use rand::seq::{SliceRandom};
use std::{thread, time};
use slack_api::{self, MessageStandard};
use slack_api::chat::PostMessageRequest;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use clap::{Arg, App, ArgMatches};
use regex::Regex;
use lazy_static;
use reqwest::blocking::Client;


pub struct RustyBot<'a> {
    emojis: Vec<&'a str>,
    id: &'a str,
    token: String,
    worker_id: String,
    tx: Sender<String>,
    is_cancelled: Arc<AtomicBool>
}


#[derive(Debug, Deserialize)]
pub struct IndexerResult {
    cycle_took: String
}


#[derive(Debug, Deserialize)]
pub struct Indexer {
    status: String,
    results: Vec<IndexerResult>
}


#[tokio::main]
pub async fn get_indexer_results(url: &str) -> Result<Indexer, reqwest::Error> {
    let indexer = format!("{}/_indexer", url);
    let json = reqwest::get(&indexer)
        .await?
	.json()
	.await?;
    Ok(json)
}


impl<'a> RustyBot<'a> {
    pub fn new(token: String, worker_id: String, tx: Sender<String>, is_cancelled: Arc<AtomicBool>) -> Self {
        RustyBot {
	    emojis: constants::EMOJIS.to_vec(),
	    id: constants::BOT_ID,
	    token: token,
	    worker_id: worker_id,
	    tx: tx,
	    is_cancelled: is_cancelled
	}
    }

    fn get_client(&self) -> Client {
	slack_api::requests::default_client().unwrap()
    }

    fn get_message(&self, channel: &'a str, text: &'a str) -> PostMessageRequest<'a> {
        PostMessageRequest {
	    channel: &channel,
	    text: &text,
	    as_user: Some(true),
	    ..Default::default()
	}
    }

    fn format_text(&self, text: &str, add_job_id: bool) -> String {
	if add_job_id {
	    return format!("{} [JOB {}]", text,  &self.worker_id);
	}
	text.to_owned()
    }

    fn say(&self, channel: &Option<String>, text: &str, add_job_id: bool) {
        slack_api::chat::post_message(
	    &self.get_client(),
	    &self.token,
	    &self.get_message(
		&self.unwrap_string(channel),
	        &self.format_text(&text, add_job_id)
	    )
	);
    }

    fn get_random_emoji(&self) -> &'a str{
        self.emojis.choose(&mut rand::thread_rng()).unwrap()
    }

    fn unwrap_string(&self, string: &'a Option<String>) -> &'a String {
        &string.as_ref().unwrap()
    }

    fn maybe_parse_slack_url(&self, url: &str) -> Option<String> {
	lazy_static! {
            static ref URL_RE: Regex = Regex::new(r"http[s]?://[a-zA-Z][0-9a-zA-Z_\.]*").unwrap();
	}
	if let Some(capture) = URL_RE.captures(url) {
	    return Some(capture.get(0).unwrap().as_str().to_owned());
	}
	None
    }

    fn should_stop(&self) -> bool {
	if self.is_cancelled.load(Ordering::Relaxed) {
	    return true;
	}
	false
    }

    fn make_app(&self) -> App {
	App::new("Rustbot")
	    .subcommand(
		App::new("status").arg(
		    Arg::with_name("url")
		)
	    ).subcommand(
		App::new("monitor").arg(
		    Arg::with_name("url")
		)
	    )
    }

    fn poll_indexer(&self, parsed_url: String, message: &MessageStandard) {
	self.say(&message.channel, &format!("START monitoring {}", &parsed_url), true);
	let mut count: usize = 0;
	loop {
	    let result = get_indexer_results(&parsed_url);
	    if let Ok(result) = result {
		if result.status == "indexing" {
		    count = 0;
		} else if result.status == "waiting" {
		    if count >= 13 {
			let value = format!("DONE monitoring {}: {:?}", &parsed_url, result);
			self.say(&message.channel, &value, true);
			break
		    }
		    count += 1;
		}
	    } else {
		self.say(&message.channel, &"Bad response, aborting", true);
		break
	    }
	    if self.should_stop() {
		println!{"Cancelling"};
		break
	    }
	    thread::sleep(time::Duration::from_secs(5));
	}
    }

    fn command_monitor(&self,  status: &ArgMatches, message: &MessageStandard) {
	self.say(&message.channel, &"Looking", true);
	if let Some(url) = status.value_of("url") {
	    let maybe_parsed_url = self.maybe_parse_slack_url(url);
	    if let Some(parsed_url) = maybe_parsed_url {
		self.poll_indexer(parsed_url, message);
		return;
	    }
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_status(&self, status: &ArgMatches, message: &MessageStandard) {
	self.say(&message.channel, &"Looking", true);
	if let Some(url) = status.value_of("url") {
	    let maybe_parsed_url = self.maybe_parse_slack_url(url);
	    if let Some(parsed_url) = maybe_parsed_url {
		let result = get_indexer_results(&parsed_url);
		if let Ok(result) = result {
		    let value = format!("{:?}", result);
		    self.say(&message.channel, &value, true);
		    thread::sleep(time::Duration::from_secs(3));
		    return;
		}
	    }
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn handle_matches(&self, matches: ArgMatches, message: &MessageStandard) {
	if let Some(status) = matches.subcommand_matches("status") {
	    self.command_status(status, &message);
	} else if let Some(monitor) = matches.subcommand_matches("monitor") {
	    self.command_monitor(monitor, &message);
	}
    }

    pub fn handle_message(&self, message: MessageStandard) {
	let text = self.unwrap_string(&message.text);
	let app = self.make_app();
	let matches = app.get_matches_from_safe(
	    text.split(' ').collect::<Vec<_>>()
	);
	match matches {
	    Ok(matches) => self.handle_matches(matches, &message),
	    Err(_) => {
		self.say(&message.channel, self.get_random_emoji(), false);
	    }
	}
    }
}


impl<'a> Drop for RustyBot<'a> {
    fn drop(&mut self) {
        println!("Dropping!");
	self.tx.send(self.worker_id.clone()).unwrap();
    }
}
