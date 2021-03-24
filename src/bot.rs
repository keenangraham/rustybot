use crate::constants;
use crate::aws::start_instance_by_url_or_id;
use crate::aws::stop_instance_by_url_or_id;
use crate::aws::resize_instance_by_url_or_id;
use crate::aws::make_ec2_client;
use crate::aws::get_instance_info_from_url_or_id;
use crate::aws::get_instance_info_from_filters;
use serde::Deserialize;
use rand::seq::{SliceRandom};
use std::{thread, time};
use std::error::Error;
use slack_api::{self, MessageStandard};
use slack_api::chat::PostMessageRequest;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use clap::{Arg, App, ArgMatches, Values};
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


#[tokio::main]
pub async fn get_visindexer_results(url: &str) -> Result<Indexer, reqwest::Error> {
    let indexer = format!("{}/_visindexer", url);
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

    fn get_url_value_and_parse(&self, matches: &ArgMatches) -> Option<String> {
	if let Some(url) = matches.value_of("url") {
	     return self.maybe_parse_slack_url(url)
	}
	None
    }

    fn get_url_or_id_value_and_parse(&self, matches: &ArgMatches) -> Option<String> {
	if let Some(url_or_id) = matches.value_of("url_or_id") {
	     return self.maybe_parse_slack_url_or_id(url_or_id)
	}
	self.get_url_value_and_parse(matches)
    }

    fn say(&self, channel: &Option<String>, text: &str, add_job_id: bool) {
        slack_api::chat::post_message(
	    &self.get_client(),
	    &self.token,
	    &self.get_message(
		&self.unwrap_string(channel),
	        &self.format_text(
		    &text.chars().take(constants::MAX_MESSAGE_SIZE).collect::<String>(),
		    add_job_id
		)
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
            static ref URL_RE: Regex = Regex::new(r"http[s]?://[a-zA-Z][-0-9a-zA-Z_\.]*").unwrap();
	}
	if let Some(capture) = URL_RE.captures(url) {
	    return Some(capture.get(0).unwrap().as_str().to_owned());
	}
	None
    }

    fn maybe_parse_slack_url_or_id(&self, url_or_id: &str) -> Option<String> {
	lazy_static! {
            static ref ID_RE: Regex = Regex::new(r"^i-[0-9][0-9a-zA-Z]*").unwrap();
	}
	if let Some(url) = self.maybe_parse_slack_url(&url_or_id) {
	    return Some(url)
	}
	if let Some(id) = ID_RE.captures(url_or_id) {
	    return Some(id.get(0).unwrap().as_str().to_owned());
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
	App::new("Rustybot")
	    .subcommand(
		App::new("status").arg(
		    Arg::with_name("url")
		)
	    ).subcommand(
		App::new("monitor").arg(
		    Arg::with_name("url")
		)
	    ).subcommand(
		App::new("vonitor").arg(
		    Arg::with_name("url")
		)
	    ).subcommand(
		App::new("konitor").arg(
		    Arg::with_name("url")
		)
	    ).subcommand(
		App::new("kronitor")
		    .arg(
			Arg::with_name("url")
		    ).arg(
			Arg::with_name("size")
			    .long("size")
			    .short("s")
			    .takes_value(true)
		    )
	    ).subcommand(
		App::new("help")
	    ).subcommand(
		App::new("ec2")
		    .subcommand(
			App::new("info").arg(
			    Arg::with_name("url_or_id")
			)
		    ).subcommand(
			App::new("start").arg(
			    Arg::with_name("url_or_id")
			)
		    ).subcommand(
			App::new("stop").arg(
			    Arg::with_name("url_or_id")
			)
		    ).subcommand(
			App::new("resize")
			    .arg(
				Arg::with_name("url_or_id")
			    )
			    .arg(
				Arg::with_name("size")
				    .long("size")
				    .short("s")
				    .takes_value(true)
			    )
		    ).subcommand(
			App::new("ls").arg(
			    Arg::with_name("filter")
				.long("filter")
				.short("f")
				.takes_value(true)
				.multiple(true)
			).arg(
			    Arg::with_name("limit")
				.long("limit")
				.short("l")
				.takes_value(true)
			)
		    )
	    )
    }

    fn poll_indexer(&self, parsed_url: String, message: &MessageStandard) -> Result<(), Box<dyn Error>> {
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
			return Ok(());
		    }
		    count += 1;
		}
	    } else {
		self.say(&message.channel, &"Bad response, aborting", true);
		return Err("Bad response".into());
	    }
	    if self.should_stop() {
		println!{"Cancelling"};
		return Err("Cancelling".into());
	    }
	    thread::sleep(time::Duration::from_secs(5));
	}
    }

    fn poll_visindexer(&self, parsed_url: String, message: &MessageStandard) -> Result<(), Box<dyn Error>> {
	self.say(&message.channel, &format!("START monitoring vis_indexer {}", &parsed_url), true);
	thread::sleep(time::Duration::from_secs(60));
	let mut count: usize = 0;
	loop {
	    let result = get_visindexer_results(&parsed_url);
	    if let Ok(result) = result {
		if result.status == "indexing" {
		    count = 0;
		} else if result.status == "waiting" {
		    if count >= 13 {
			let value = format!("DONE monitoring vis_indexer {}: {:?}", &parsed_url, result);
			self.say(&message.channel, &value, true);
			return Ok(());
		    }
		    count += 1;
		}
	    } else {
		self.say(&message.channel, &"Bad response, aborting", true);
		return Err("Bad response".into());
	    }
	    if self.should_stop() {
		println!{"Cancelling"};
		return Err("Cancelling".into());
	    }
	    thread::sleep(time::Duration::from_secs(5));
	}
    }

    fn command_monitor(&self,  monitor: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url) = self.get_url_value_and_parse(monitor) {
	    self.poll_indexer(parsed_url, message);
	    return;
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_vonitor(&self,  vonitor: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url) = self.get_url_value_and_parse(vonitor) {
	    self.poll_visindexer(parsed_url, message);
	    return;
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_konitor(&self,  konitor: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url) = self.get_url_value_and_parse(konitor) {
	    let polling = self.poll_indexer(parsed_url.to_owned(), message);
	    if polling.is_err() {
		return;
	    }
	    if self.should_stop() {
		println!{"Cancelling"};
		return;
	    }
	    let vispolling = self.poll_visindexer(parsed_url.to_owned(), message);
	    if vispolling.is_err() {
		return;
	    }
	    if self.should_stop() {
		println!{"Cancelling"};
		return;
	    }
	    self.command_ec2_stop(konitor, message);
	    return;
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_kronitor(&self,  kronitor: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url) = self.get_url_value_and_parse(kronitor) {
	    let polling = self.poll_indexer(parsed_url.to_owned(), message);
	    if polling.is_err() {
		return;
	    }
	    if self.should_stop() {
		println!{"Cancelling"};
		return;
	    }
	    let vispolling = self.poll_visindexer(parsed_url.to_owned(), message);
	    if vispolling.is_err() {
		return;
	    }
	    if self.should_stop() {
		println!{"Cancelling"};
		return;
	    }
	    self.command_ec2_stop(kronitor, message);
	    if self.should_stop() {
		println!{"Cancelling"};
		return;
	    }
	    self.say(&message.channel, &"Waiting to resize", true);
	    thread::sleep(time::Duration::from_secs(60));
	    if self.should_stop() {
		println!{"Cancelling"};
		return;
	    }
	    self.command_ec2_resize(kronitor, message);
	    return;
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_status(&self, status: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url) = self.get_url_value_and_parse(status) {
	    let result = get_indexer_results(&parsed_url);
	    if let Ok(result) = result {
		let value = format!("{:?}", result);
		self.say(&message.channel, &value, true);
		thread::sleep(time::Duration::from_secs(3));
		return;
	    }
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_help(&self,  help: &ArgMatches, message: &MessageStandard) {
	self.say(&message.channel, constants::HELP, false);
    }

    fn command_ec2_info(&self, info: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url_or_id) = self.get_url_or_id_value_and_parse(info) {
	    let ec2 = make_ec2_client();
	    let instance_info = get_instance_info_from_url_or_id(
		&ec2,
		parsed_url_or_id.clone()
	    );
	    if !instance_info.is_empty() {
		self.say(&message.channel, &format!("Getting instance info for {}", &parsed_url_or_id), true);
		let value = format!("{:?}", instance_info);
		self.say(&message.channel, &value, true);
		thread::sleep(time::Duration::from_secs(3));
		return;
	    }
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_ec2_start(&self, start: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url_or_id) = self.get_url_or_id_value_and_parse(start) {
	    let ec2 = make_ec2_client();
	    let started_instance = start_instance_by_url_or_id(
		&ec2,
		parsed_url_or_id.clone()
	    );
	    if let Ok(started_instance) = started_instance {
		self.say(&message.channel, &format!("Starting instance {}", &parsed_url_or_id), true);
		let value = format!("{:?}", started_instance);
		self.say(&message.channel, &value, true);
		thread::sleep(time::Duration::from_secs(3));
		return;
	    }
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_ec2_stop(&self, stop: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url_or_id) = self.get_url_or_id_value_and_parse(stop) {
	    let ec2 = make_ec2_client();
	    let stopped_instance = stop_instance_by_url_or_id(
		&ec2,
		parsed_url_or_id.clone()
	    );
	    if let Ok(stopped_instance) = stopped_instance {
		self.say(&message.channel, &format!("Stopping instance {}", &parsed_url_or_id), true);
		let value = format!("{:?}", stopped_instance);
		self.say(&message.channel, &value, true);
		thread::sleep(time::Duration::from_secs(3));
		return;
	    }
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_ec2_resize(&self, resize: &ArgMatches, message: &MessageStandard) {
	if let Some(parsed_url_or_id) = self.get_url_or_id_value_and_parse(resize) {
	    let ec2 = make_ec2_client();
	    let size = resize.value_of("size").unwrap_or(
		constants::RESIZE_INSTANCE
	    );
	    let resized_instance = resize_instance_by_url_or_id(
		&ec2,
		parsed_url_or_id.clone(),
		size.to_owned(),
	    );
	    match resized_instance {
		Ok(_) => {
		    let value = format!(
			"Resized instance {} to {}: {:?}",
			&parsed_url_or_id,
			&size,
			get_instance_info_from_url_or_id(&ec2, parsed_url_or_id.clone())
		    );
		    self.say(&message.channel, &value, true);
		    thread::sleep(time::Duration::from_secs(3));
		    return;
		}
		Err(error) => {
		    let value = format!("{}", error);
		    self.say(&message.channel, &value, true);
		    thread::sleep(time::Duration::from_secs(3));
		    return;
		}
	    }
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn command_ec2_ls(&self, list: &ArgMatches, message: &MessageStandard) {
        let ec2 = make_ec2_client();
	let filters = list.values_of("filter")
	    .unwrap_or(Values::default())
	    .map(
		|x| {
		    let values = x.split("=").collect::<Vec<_>>();
		    if values.len() == 2 {
			return Some((values[0].to_string(), values[1].to_string()));
		    } else {
			return None;
		    }
		}
	    )
	    .filter(|x| x.is_some())
	    .map(|x| x.unwrap())
	    .collect::<Vec<_>>();
	let limit = list.value_of("limit").unwrap_or("3").parse::<usize>().unwrap_or(3);
        let matching_instances = get_instance_info_from_filters(&ec2, filters);
	if let Ok(matches) = matching_instances {
	    let results =  matches.iter().enumerate().collect::<Vec<_>>();
	    let total = results.len();
	    let value = format!(
		"Showing {} out of {}:\n{:?}",
		if limit <= total {limit} else {total},
		results.len(),
		results.iter().take(limit).collect::<Vec<_>>()
	    );
	    self.say(&message.channel, &value, true);
	    thread::sleep(time::Duration::from_secs(3));
	    return;
	}
	self.say(&message.channel, &"Bad input", true);
    }

    fn handle_matches(&self, matches: ArgMatches, message: &MessageStandard) {
	match matches.subcommand() {
	    ("status", Some(status)) => self.command_status(status, &message),
	    ("monitor", Some(monitor)) => self.command_monitor(monitor, &message),
	    ("vonitor", Some(vonitor)) => self.command_vonitor(vonitor, &message),
	    ("konitor", Some(konitor)) => self.command_konitor(konitor, &message),
	    ("kronitor", Some(kronitor)) => self.command_kronitor(kronitor, &message),
	    ("help", Some(help)) => self.command_help(help, &message),
	    ("ec2", Some(ec2)) => {
		match ec2.subcommand() {
		    ("info", Some(info)) => self.command_ec2_info(info, &message),
		    ("start", Some(start)) => self.command_ec2_start(start, &message),
		    ("stop", Some(stop)) => self.command_ec2_stop(stop, &message),
		    ("resize", Some(resize)) => self.command_ec2_resize(resize, &message),
		    ("ls", Some(ls)) => self.command_ec2_ls(ls, &message),
 		    _ => ()
		}
	    },
	    _ => ()
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
	    Err(error) => {
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
