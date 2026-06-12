use crate::prelude::*;

mod hypr;

#[derive(Debug)]
pub struct Window {
	class: String,
	title: String,
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
				return Err(eyre!("{s} is unknown"));
			}
		},
	);
}
