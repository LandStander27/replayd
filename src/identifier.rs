use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::prelude::*;

mod hypr;

static IDENTIFIABLE_GAMES: OnceCell<Vec<IdentifiableGame>> = OnceCell::const_new();

#[derive(Debug, Clone, Deserialize)]
pub struct IdentifiableGame {
	pub class: String,
	pub title_substring: String,
	pub name: String,
}

pub async fn get_games() -> Result<()> {
	let client = reqwest::Client::builder()
		.timeout(std::time::Duration::from_secs(2))
		.build()
		.context("could not build client")?;
	let response = client
		.get("https://cdn.landsj.dev/games.jsonc")
		.send()
		.await
		.context("could not send request")?;
	let json: String = response
		.text()
		.await
		.context("could not get text from request")?
		.lines()
		.filter(|x| !x.starts_with("//"))
		.collect();
	let parsed: Vec<IdentifiableGame> = serde_json::from_str(&json).context("could not parse json from request")?;

	IDENTIFIABLE_GAMES
		.set(parsed)
		.context("could not set IDENTIFIABLE_GAMES")?;

	return Ok(());
}

pub fn identify_game(window: &Window) -> Option<IdentifiableGame> {
	let games = if IDENTIFIABLE_GAMES.initialized() {
		IDENTIFIABLE_GAMES.get().unwrap()
	} else {
		&Default::default()
	};

	return games
		.iter()
		.find(|x| x.class == window.class && window.title.contains(&x.title_substring))
		.cloned();
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
