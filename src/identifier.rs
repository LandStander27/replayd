use std::collections::HashMap;
use tokio::sync::OnceCell;

use crate::prelude::*;

mod hypr;

static IDENTIFIABLE_GAMES: OnceCell<HashMap<String, String>> = OnceCell::const_new();

pub async fn get_games() -> Result<()> {
	let client = reqwest::Client::builder()
		.timeout(std::time::Duration::from_secs(2))
		.build()
		.context("could not build client")?;
	let response = client
		.get("https://cdn.landsj.dev/games.json")
		.send()
		.await
		.context("could not send request")?;
	let json = response
		.text()
		.await
		.context("could not get text from request")?;
	let parsed: HashMap<String, String> = serde_json::from_str(&json).context("could not parse json from request")?;

	IDENTIFIABLE_GAMES
		.set(parsed)
		.context("could not set IDENTIFIABLE_GAMES")?;

	return Ok(());
}

pub fn identify_game(class: impl AsRef<str>) -> Option<String> {
	let games = if IDENTIFIABLE_GAMES.initialized() {
		IDENTIFIABLE_GAMES.get().unwrap()
	} else {
		&Default::default()
	};

	return games.get(class.as_ref()).cloned();
}

#[derive(Debug)]
pub struct Window {
	pub class: String,
	pub title: String,
	pub fullscreen: bool,
}

#[async_trait]
pub trait WindowManager {
	async fn get_focused_window(&self) -> Result<Window>;
}

pub struct UnknownWindowManager;

#[async_trait]
impl WindowManager for UnknownWindowManager {
	async fn get_focused_window(&self) -> Result<Window> {
		return Err(eyre!("unknown window manager"));
	}
}

pub fn get_window_manager() -> Result<Box<dyn WindowManager>> {
	return Ok(
		match std::env::var("XDG_CURRENT_DESKTOP")
			.context("XDG_CURRENT_DESKTOP does not exist")?
			.as_str()
		{
			"Hyprland" => Box::new(hypr::Hyprland),
			s => {
				return Err(eyre!("{s} is unknown")); // TODO: add more
			}
		},
	);
}
