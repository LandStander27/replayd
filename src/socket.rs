use crate::prelude::*;

pub struct Listener {
	shutdown: Arc<Notify>,
	thread: JoinHandle<()>,
}

impl Listener {
	pub fn bind(tx: relm4::Sender<Message>, #[cfg(feature = "socket_commands")] db: Db) -> Result<Self> {
		let socket_path = dirs::runtime_dir()
			.context("could not find XDG_RUNTIME_DIR")?
			.join("replayd")
			.join("socket.sock");

		if socket_path.exists() {
			std::fs::remove_file(&socket_path).context("could not delete socket file")?;
		}
		std::fs::create_dir_all(
			socket_path
				.parent()
				.context("could not get socket parent directory")?,
		)
		.context("could not create socket path")?;

		let shutdown: Arc<Notify> = Arc::default();
		let listener = UnixListener::bind(&socket_path).context("could not bind to socket")?;

		let thread = {
			let shutdown = shutdown.clone();
			tokio::spawn(async move {
				loop {
					let accept;
					tokio::select! {
						_ = shutdown.notified() => break,
						x = listener.accept() => accept = x.context("could not accept unix connection"),
					}

					let tx = tx.clone();

					#[cfg(feature = "socket_commands")]
					let db = db.clone();
					match accept {
						Err(e) => {
							error!(?e);
							tx.emit(Message::Error(format!("{e:#}")));
						}
						Ok((mut stream, _)) => {
							tokio::spawn(async move {
								let mut buf = String::new();
								use tokio::io::AsyncReadExt;
								if let Err(e) = stream
									.read_to_string(&mut buf)
									.await
									.context("could not read stream")
								{
									error!(?e);
									tx.emit(Message::Error(format!("{e:#}")));
								}

								#[cfg(feature = "socket_commands")]
								match handle_command(&buf, &mut stream, db, &tx).await {
									Ok(true) => return,
									Ok(false) => {}
									Err(e) => {
										error!(?e);
										tx.emit(Message::Error(format!("{e:#}")));
									}
								}

								tx.emit(Message::ClipReceived(PathBuf::from(buf)));
							});
						}
					}
				}
			})
		};

		return Ok(Self { shutdown, thread });
	}

	pub async fn shutdown(self) -> Result<()> {
		self.shutdown.notify_one();
		self.thread.await.context("thread panicked")?;

		std::fs::remove_file(
			dirs::runtime_dir()
				.context("could not find XDG_RUNTIME_DIR")?
				.join("replayd")
				.join("socket.sock"),
		)
		.context("could not delete socket file")?;

		return Ok(());
	}
}

#[cfg(feature = "socket_commands")]
async fn handle_command(buf: &str, stream: &mut UnixStream, db: Db, tx: &relm4::Sender<Message>) -> Result<bool> {
	use std::fmt::Write;
	match buf.trim() {
		"get/db" => {
			let mut s = String::new();
			writeln!(
				&mut s,
				r#"
schema version: {:#?}

settings: {:#?}

games: {:#?}

clips: {:#?}
"#,
				db.schema_version()?,
				db.read_settings()?,
				db.get_games()?,
				db.get_clips()?
			)?;
			stream.write_all(s.as_bytes()).await?;
		}
		"get/games" => {
			stream
				.write_all(format!("{:#?}\n", identifier::get_all_games()).as_bytes())
				.await?;
		}
		"signal/clip" => tx.emit(Message::SaveClip),
		"signal/toggle" => tx.emit(Message::ToggleClipping),
		_ => return Ok(false),
	}

	return Ok(true);
}
