use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::prelude::*;

mod hypr;

static IDENTIFIABLE_GAMES: OnceCell<Vec<IdentifiableGame>> = OnceCell::const_new();

#[derive(Debug, Clone, Deserialize)]
pub struct Executable {
	pub binary: String,

	#[serde(default)]
	pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IdentifiableGame {
	pub id: u64,
	pub name: String,

	#[serde(default)]
	pub classes: Vec<String>,

	#[serde(default)]
	pub title_substring: Option<String>,

	#[serde(default)]
	pub executables: Vec<Executable>,
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
		.filter(|x| !x.starts_with("//")) // why does json not support comments !!!!!!!
		.collect();
	let parsed: Vec<IdentifiableGame> = serde_json::from_str(&json).context("could not parse json from request")?;

	IDENTIFIABLE_GAMES
		.set(parsed)
		.context("could not set IDENTIFIABLE_GAMES")?;

	return Ok(());
}

pub fn get_game(id: ObjectId) -> Option<&'static IdentifiableGame> {
	return IDENTIFIABLE_GAMES.get().and_then(|x| x.get(id as usize));
}

#[cfg(feature = "socket_commands")]
pub fn get_all_games() -> &'static Vec<IdentifiableGame> {
	return IDENTIFIABLE_GAMES.get().unwrap();
}

pub fn identify_game(window: &Window) -> Option<ObjectId> {
	let games = if IDENTIFIABLE_GAMES.initialized() {
		IDENTIFIABLE_GAMES.get().unwrap()
	} else {
		&Default::default()
	};

	return games
		.iter()
		.find(|game| {
			let args = window.cmdline.join(" ");
			if !game.executables.is_empty()
				&& let Some(ref exe) = window.executable
			{
				let exe_matches = game.executables.iter().any(|e| {
					if e.binary != *exe {
						return false;
					}

					e.arguments.iter().all(|arg| args.contains(arg.as_str()))
				});

				if exe_matches {
					return true;
				}
			}

			if !game.classes.is_empty() {
				let class_matches = game.classes.iter().any(|c| c == &window.class);
				if class_matches {
					return match &game.title_substring {
						Some(sub) => window.title.contains(sub.as_str()),
						None => return true,
					};
				}
			}

			return false;
		})
		.map(|x| x.id);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
	pub class: String,
	pub title: String,
	pub executable: Option<String>,
	pub cmdline: Vec<String>,
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
			"KDE" => todo!("will have to use kdotool"),
			"GNOME" => todo!("will have to write a gnome extension that opens a socket or something "),
			s => {
				return Err(eyre!("{s} is unknown")); // TODO: add more
			}
		},
	);
}
