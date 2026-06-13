use crate::prelude::*;

pub struct Listener {
	shutdown: Arc<Notify>,
	thread: JoinHandle<()>,
}

impl Listener {
	pub fn bind(tx: relm4::Sender<Message>, db: Db) -> Result<Self> {
		let socket_path = dirs::runtime_dir()
			.context("could not find XDG_RUNTIME_DIR")?
			.join("replayd")
			.join("hook.sock");

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

								if buf.trim() == "\\settings" {
									if let Err(e) = stream
										.write_all(format!("{:#?}", db.read_settings().unwrap()).as_bytes())
										.await
										.context("could not write to stream")
									{
										error!(?e);
										tx.emit(Message::Error(format!("{e:#}")));
									}
								} else {
									match url::Url::from_file_path(&buf)
										.ok()
										.context("could not create file uri")
									{
										Ok(o) => tx.emit(Message::ClipReceived {
											path: PathBuf::from(buf),
											path_uri: o.to_string(),
										}),
										Err(e) => {
											error!(?e);
											tx.emit(Message::Error(format!("{e:#}")));
										}
									}
								}
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
				.join("hook.sock"),
		)
		.context("could not delete socket file")?;

		return Ok(());
	}
}
