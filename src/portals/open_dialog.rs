use crate::prelude::*;

use ashpd::{
	Error::Response,
	WindowIdentifier,
	desktop::{ResponseError, file_chooser::OpenFileRequest},
};

#[derive(Default)]
pub struct OpenFileDialog<T: IsA<gtk::Native> + Default> {
	window: T,
	title: String,
	dir: bool,
}

impl<T: IsA<gtk::Native> + Default> OpenFileDialog<T> {
	pub fn new() -> Self {
		return Self::default();
	}

	pub fn window(mut self, window: T) -> Self {
		self.window = window;
		return self;
	}

	pub fn title(mut self, title: impl Into<String>) -> Self {
		self.title = title.into();
		return self;
	}

	pub fn dir(mut self, dir: bool) -> Self {
		self.dir = dir;
		return self;
	}

	pub async fn open(self) -> Result<Option<Vec<PathBuf>>> {
		let ident = WindowIdentifier::from_native(&self.window)
			.await
			.context("could not get identify window")?;

		let request = OpenFileRequest::default()
			.directory(self.dir)
			.identifier(ident)
			.modal(true)
			.title(self.title.as_str());

		let files = match request.send().await.and_then(|r| r.response()) {
			Ok(o) => o,
			Err(Response(ResponseError::Cancelled)) => return Ok(None),
			Err(e) => return Err(e).context("could not get response from FileChooser portal"),
		};

		let files: Result<Vec<PathBuf>> = files
			.uris()
			.iter()
			.map(|x| {
				url::Url::parse(x.as_str())
					.context("invalid uri")
					.and_then(|x| x.to_file_path().ok().context("invalid file uri"))
			})
			.collect();

		return Ok(Some(files?));
	}
}
