use crate::prelude::*;

use ashpd::{
	WindowIdentifier,
	desktop::open_uri::{OpenDirOptions, OpenURIProxy},
};

pub async fn open_directory(window: &impl IsA<gtk::Native>, path: File) -> Result<()> {
	let ident = WindowIdentifier::from_native(window)
		.await
		.context("could not get identify window")?;

	let proxy = OpenURIProxy::new()
		.await
		.context("could not get OpenURIProxy")?;

	proxy
		.open_directory(Some(&ident), &path, OpenDirOptions::default())
		.await
		.context("could not open directory")?;

	return Ok(());
}

// pub async fn open_file(window: &impl IsA<gtk::Native>, path: File) -> Result<()> {
// 	let ident = WindowIdentifier::from_native(window)
// 		.await
// 		.context("could not get identify window")?;

// 	let proxy = OpenURIProxy::new()
// 		.await
// 		.context("could not get OpenURIProxy")?;

// 	let mut options = OpenFileOptions::default();
// 	options.ask = Some(true);
// 	proxy
// 		.open_file(Some(&ident), &path, options)
// 		.await
// 		.context("could not open directory")?;

// 	return Ok(());
// }
