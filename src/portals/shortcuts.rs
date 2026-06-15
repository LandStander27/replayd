use crate::prelude::*;

use ashpd::{
	Error::Response,
	WindowIdentifier,
	desktop::{
		CreateSessionOptions, ResponseError, Session,
		global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
	},
};
use futures::StreamExt;

pub struct ShortcutsSession {
	session: Session<GlobalShortcuts>,
	shutdown: Arc<Notify>,
	thread: JoinHandle<()>,
}

impl ShortcutsSession {
	pub async fn start(tx: relm4::Sender<Message>, window: &impl IsA<gtk::Native>) -> Result<Self> {
		let ident = WindowIdentifier::from_native(window)
			.await
			.context("could not get WindowIdentifier")
			.inspect_err(|e| warn!(?e))
			.ok();

		let global_shortcuts = GlobalShortcuts::new()
			.await
			.context("could not get GlobalShortcuts portal")?;
		let session = global_shortcuts
			.create_session(CreateSessionOptions::default())
			.await
			.context("could not create GlobalShortcuts session")?;
		let shortcuts = [
			NewShortcut::new("clip", "Save a clip"),
			NewShortcut::new("toggle-clipping", "Toggle screen recording for clipping"),
		];
		let request = global_shortcuts
			.bind_shortcuts(&session, &shortcuts, ident.as_ref(), BindShortcutsOptions::default())
			.await
			.context("could not bind shortcuts")?;
		match request.response() {
			Ok(_) => {}
			Err(Response(ResponseError::Cancelled)) => return Err(eyre!("shortcut binding was cancelled")),
			Err(e) => return Err(eyre!("{e}")),
		}

		let shutdown: Arc<Notify> = Arc::default();
		let thread = {
			let shutdown = shutdown.clone();
			tokio::spawn(async move {
				let mut stream = match global_shortcuts
					.receive_activated()
					.await
					.context("could not get shortcuts stream")
				{
					Ok(o) => o,
					Err(e) => {
						error!(?e);
						tx.emit(Message::Error(format!("{e:#}")));
						return;
					}
				};

				loop {
					let activated;
					tokio::select! {
						_ = shutdown.notified() => break,
						Some(x) = stream.next() => activated = x,
					}

					match activated.shortcut_id() {
						"clip" => tx.emit(Message::SaveClip),
						"toggle-clipping" => tx.emit(Message::ToggleClipping),
						_ => continue,
					}
				}
			})
		};

		return Ok(Self { session, shutdown, thread });
	}

	pub async fn shutdown(self) -> Result<()> {
		self.shutdown.notify_one();
		self.thread.await.context("thread panicked")?;
		self.session
			.close()
			.await
			.context("could not shutdown global shortcuts session")?;
		return Ok(());
	}
}
