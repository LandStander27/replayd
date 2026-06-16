use std::str::FromStr;

pub mod open_dialog;
pub mod open_uri;
pub mod shortcuts;

use crate::prelude::*;

pub async fn register() -> Result<()> {
	let appid = ashpd::AppID::from_str("dev.land.Replayd")?;
	ashpd::register_host_app(appid).await?;

	return Ok(());
}
