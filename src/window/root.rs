use ksni::TrayMethods;

use crate::prelude::*;

struct App {
	window: adw::ApplicationWindow,
	error_dialog: ErrorDialog<adw::ApplicationWindow>,
	tray: ksni::Handle<Tray>,
	visible: bool,
	additional_css_class: Option<&'static str>,
	games: FactoryVecDeque<GameChip>,
	db: Db,
	recorder: Recorder,
	window_manager: Box<dyn WindowManager>,
	socket_listener: Option<Listener>,
	shortcuts: Option<ShortcutsSession>,
	settings: Settings,
}

#[derive(Debug)]
pub enum Message {
	Void,
	LoadSettings,
	Close,
	ShowWindow,
	Exit,
	Init,
	Error(String),
	GamesLoaded(Vec<db::Game>),
	GameSelected(DynamicIndex),
	ClipReceived(String),
	SaveClip,
	ToggleClipping,
	PickOutputDir,
	SetOutputDir(PathBuf),
}

#[relm4::component(async)]
impl AsyncComponent for App {
	type Init = String;
	type Input = Message;
	type Output = ();
	type CommandOutput = Message;

	view! {
		stack = &adw::ViewStack {
			add_titled_with_icon[Some("home"), "Home", "go-home-symbolic"] = &gtk::Box {
				set_orientation: gtk::Orientation::Vertical,
				set_spacing: 8,

				adw::Clamp {
					set_maximum_size: 600,
					set_margin_top: 12,
					set_margin_start: 12,
					set_margin_end: 12,

					gtk::SearchEntry {
						set_placeholder_text: Some("Search clips..."),
						add_css_class: "pill",
					},
				},

				gtk::ScrolledWindow {
					set_hexpand: true,
					set_vscrollbar_policy: gtk::PolicyType::Never,
					set_hscrollbar_policy: gtk::PolicyType::Automatic,
					set_margin_start: 12,
					set_margin_end: 12,

					#[local_ref]
					games_box -> gtk::Box {
						set_orientation: gtk::Orientation::Horizontal,
						set_spacing: 8,
						set_margin_top: 4,
						set_margin_bottom: 4,
					}
				},

				gtk::Box {
					set_orientation: gtk::Orientation::Horizontal,
					set_spacing: 12,
					set_margin_top: 8,
					set_margin_start: 12,
					set_margin_end: 12,
					add_css_class: "card",

					// red dot
					gtk::Image {
						set_icon_name: Some("media-record-symbolic"),
						#[watch]
						set_visible: app.recorder.is_active(),
						add_css_class: "error", // gives it the red accent color
					},

					// status text
					gtk::Box {
						set_orientation: gtk::Orientation::Vertical,
						set_valign: gtk::Align::Center,
						set_hexpand: true,
						set_spacing: 2,

						gtk::Label {
							set_label: if app.recorder.is_active() { "Replay buffer active" } else { "Replay buffer inactive" },
							set_halign: gtk::Align::Start,
							add_css_class: "heading",
						},

						gtk::Label {
							#[watch]
							set_label: &format!("{}p · {} fps · {}", app.settings.resolution, app.settings.frame_rate, app.settings.codec),
							set_halign: gtk::Align::Start,
							add_css_class: "caption",
							add_css_class: "dim-label",
						},
					},

					gtk::Button {
						set_label: "Save a clip",

						#[watch]
						set_visible: app.recorder.is_active(),

						#[watch]
						set_css_classes: &["suggested-action", "pill"],

						set_valign: gtk::Align::Center,
						set_margin_top: 8,
						set_margin_bottom: 8,

						connect_clicked => Message::SaveClip,
					},

					gtk::Button {
						#[watch]
						set_label: if app.recorder.is_active() { "Disable" } else { "Enable" },

						#[watch]
						set_css_classes: &[if app.recorder.is_active() { "destructive-action" } else { "suggested-action" }, "pill"],

						set_valign: gtk::Align::Center,
						set_margin_top: 8,
						set_margin_bottom: 8,

						connect_clicked => Message::ToggleClipping,
					}
				}
			},
			add_titled_with_icon[Some("settings"), "Settings", "emblem-system-symbolic"] = &gtk::Box {
				adw::PreferencesPage {
					adw::PreferencesGroup {
						set_title: "Replay buffer",
						adw::SwitchRow {
							set_title: "Enable replay buffer", // TODO: maybe remove
							set_active: false,
						},

						adw::SpinRow {
							set_title: "Buffer length",
							set_subtitle: "How many minutes to keep in memory",
							#[wrap(Some)]
							set_adjustment = &gtk::Adjustment {
								set_lower: 1.0,
								set_upper: 60.0,
								set_step_increment: 1.0,
								set_page_increment: 5.0,
								set_page_size: 0.0,
							},
							set_digits: 2,

							set_value: app.settings.buffer_length as f64 / 60.0,

							connect_changed[db, sender] => move |x| {
								let value = (x.value() * 60.0).round() as i32;
								if let Err(e) = db.write_settings(|s| s.buffer_length = value) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							}
						},

						#[name(displays)]
						adw::ComboRow {
							set_title: "Display",

							connect_selected_notify[db, sender] => move |x| {
								let value = x.selected_item().and_downcast::<gtk::StringObject>().map(|x| x.string().to_string());
								if let Err(e) = db.write_settings(|s| s.display = value) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							}
						}
					},
					adw::PreferencesGroup {
						set_title: "Encoding",

						adw::ComboRow {
							set_title: "Quality",
							set_model: Some(&gtk::StringList::new(&["Medium", "High", "Very high", "Ultra"])),
							set_selected: app.settings.quality as u32,
							connect_selected_notify[db, sender] => move |x| {
								let value = x.selected() as usize;
								if let Err(e) = db.write_settings(|s| s.quality = Quality::from_repr(value).unwrap()) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							}
						},

						adw::ComboRow {
							set_title: "Encoder",
							set_model: Some(&gtk::StringList::new(&["H.264", "AV1", "HEVC"])),
							set_selected: app.settings.codec as u32,
							connect_selected_notify[db, sender] => move |x| {
								let value = x.selected() as usize;
								if let Err(e) = db.write_settings(|s| s.codec = Codec::from_repr(value).unwrap()) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							}
						},

						adw::ComboRow {
							set_title: "Resolution",
							set_model: Some(&gtk::StringList::new(&["1440p", "1080p", "720p"])),
							set_selected: app.settings.resolution as u32,
							connect_selected_notify[db, sender] => move |x| {
								let value = x.selected() as usize;
								if let Err(e) = db.write_settings(|s| s.resolution = Resolution::from_repr(value).unwrap()) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							}
						},

						adw::ComboRow {
							set_title: "Frame rate",
							set_model: Some(&gtk::StringList::new(&["24 fps", "30 fps", "60 fps", "120 fps", "144 fps"])),
							set_selected: app.settings.frame_rate as u32,
							connect_selected_notify[db, sender] => move |x| {
								let value = x.selected() as usize;
								if let Err(e) = db.write_settings(|s| s.frame_rate = FrameRate::from_repr(value).unwrap()) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							}
						},
					},
					adw::PreferencesGroup {
						set_title: "Storage",
						adw::ActionRow {
							set_title: "Clip save location",

							#[watch]
							set_subtitle: &app.settings.output_dir,

							add_suffix = &gtk::Button {
								set_label: "Change",
								set_valign: gtk::Align::Center,
								add_css_class: "flat",
								connect_clicked => Message::PickOutputDir
							},
						},
					}
				}
			},

			set_enable_transitions: true,
		},

		#[root]
		main_window = adw::ApplicationWindow {
			set_title: Some(&title),
			add_css_class?: app.additional_css_class,

			#[watch]
			set_visible: app.visible,

			connect_close_request[sender] => move |_| {
				sender.input(Message::Close);
				return glib::Propagation::Stop;
			},

			adw::ToolbarView {
				#[name(header_bar)]
				add_top_bar = &adw::HeaderBar {
					#[wrap(Some)]
					set_title_widget = &adw::ViewSwitcher {
						set_stack: Some(&stack),
						set_policy: adw::ViewSwitcherPolicy::Wide,
					},
				},

				#[wrap(Some)]
				set_content = &stack.clone(),

				#[name(switcher_bar)]
				add_bottom_bar = &adw::ViewSwitcherBar {
					set_stack: Some(&stack),
				}
			},
		},
	}

	async fn init(title: Self::Init, root: Self::Root, sender: AsyncComponentSender<Self>) -> AsyncComponentParts<Self> {
		let error_dialog = ErrorDialog::new(root.clone());

		let (db, settings) = match Db::open() {
			Ok(db) => {
				let (db, settings) = match db.read_settings() {
					Ok(s) => (db, s),
					Err(e) => {
						error!(?e);
						error_dialog.show(format!("{e:#}"));
						(Db::memory().unwrap(), Settings::default())
					}
				};

				(db, settings)
			}
			Err(e) => {
				error!(?e);
				error_dialog.show(format!("{e:#}"));
				(Db::memory().unwrap(), Settings::default())
			}
		};
		info!("db init");

		let listener = match Listener::bind(sender.input_sender().clone(), db.clone()) {
			Ok(o) => Some(o),
			Err(e) => {
				error!(?e);
				error_dialog.show(format!("{e:#}"));
				None
			}
		};

		let window_manager = match identifier::get_window_manager() {
			Ok(o) => o,
			Err(e) => {
				error!(?e);
				error_dialog.show(format!("{e:#}"));
				Box::new(identifier::UnknownWindowManager)
			}
		};

		let shortcuts = match ShortcutsSession::start(sender.input_sender().clone(), &root).await {
			Ok(o) => Some(o),
			Err(e) => {
				error!(?e);
				error_dialog.show(format!("{e:#}"));
				None
			}
		};

		let app = App {
			window: root.clone(),
			error_dialog,
			visible: true,
			tray: match Tray::new(sender.input_sender().clone()).spawn().await {
				Ok(o) => o,
				Err(e) => {
					error!(?e);
					std::process::exit(1);
				}
			},
			additional_css_class: if cfg!(debug_assertions) {
				Some("devel")
			} else {
				None
			},
			games: FactoryVecDeque::builder()
				.launch(gtk::Box::default())
				.forward(sender.input_sender(), |a| a),
			db: db.clone(),
			recorder: Recorder::new(),
			socket_listener: listener,
			window_manager,
			shortcuts,
			settings,
		};

		let games_box = app.games.widget();
		let mut widgets = view_output!();

		let bp = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
			adw::BreakpointConditionLengthType::MaxWidth,
			550.0,
			adw::LengthUnit::Sp,
		));
		bp.add_setter(&widgets.switcher_bar, "reveal", Some(&true.to_value()));
		bp.add_setter(&widgets.header_bar, "title-widget", Some(&Option::<&gtk::Widget>::None.to_value()));
		widgets.main_window.add_breakpoint(bp);

		if let Err(e) = app.load_displays(&mut widgets.displays) {
			error!(?e);
			sender.input(Message::Error(format!("{e:#}")));
		}

		sender.input(Message::Init);

		return AsyncComponentParts { model: app, widgets };
	}

	#[tracing::instrument(skip(self, sender))]
	async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>, _window: &adw::ApplicationWindow) {
		if let Err(e) = self.update(msg, sender).await {
			error!(?e);
			self.error_dialog.show(format!("{e:#}"));
		}
	}
}

impl App {
	fn load_displays(&self, widget: &mut adw::ComboRow) -> Result<()> {
		info!("loading displays");
		let displays = gtk::StringList::new(&[]);
		let v = recorder::get_displays().context("could not get displays")?;

		for display in v.iter() {
			displays.append(display);
		}

		// let initial = &self.settings.display;
		widget.set_model(Some(&displays));
		if let Some(initial) = &self.settings.display {
			widget.set_selected(v.iter().position(|x| x == initial).unwrap_or_default() as u32);
		}

		return Ok(());
	}

	async fn update(&mut self, msg: Message, sender: AsyncComponentSender<Self>) -> Result<()> {
		let tx = sender.input_sender();
		match msg {
			Message::Void => {}
			Message::SetOutputDir(dir) => {
				self.db
					.write_settings(move |settings| settings.output_dir = dir.display().to_string())?;
				tx.emit(Message::LoadSettings);
			}
			Message::PickOutputDir => {
				let window = self.window.clone();
				let tx = tx.clone();
				relm4::spawn_local(async move {
					tx.emit(
						match OpenFileDialog::new()
							.window(window)
							.dir(true)
							.title("Select Clips folder")
							.open()
							.await
						{
							Ok(Some(mut file)) => Message::SetOutputDir(std::mem::take(file.first_mut().unwrap())),
							Ok(None) => Message::Void,
							Err(e) => Message::Error(format!("{e:#}")),
						},
					);
				});
			}
			Message::LoadSettings => self.settings = self.db.read_settings()?,
			Message::Error(e) => self.error_dialog.show(e),
			Message::Close => self.visible = false,
			Message::ShowWindow => self.visible = true,
			Message::Exit => {
				if self.recorder.is_active() {
					self.recorder.stop().context("could not stop recording")?;
				}
				if let Some(listener) = self.socket_listener.take() {
					listener
						.shutdown()
						.await
						.context("failed to shutdown socket")?;
				}
				if let Some(shortcuts) = self.shortcuts.take() {
					shortcuts
						.shutdown()
						.await
						.context("failed to shutdown shortcuts")?;
				}
				self.tray.shutdown().await;
				relm4::main_application().quit();
			}
			Message::Init => {
				info!("loading games");
				tx.emit(Message::GamesLoaded(self.db.get_games()?));
			}
			Message::GamesLoaded(games) => {
				let mut guard = self.games.guard();
				guard.clear();
				guard.push_back("All games".to_string());
				for game in games {
					guard.push_back(game.window_class);
				}
				guard.send(0, true);
			}
			Message::GameSelected(index) => {
				let guard = self.games.guard();
				for i in 0..guard.len() {
					guard.send(i, i == index.current_index());
				}
			}
			Message::SaveClip => todo!(),
			Message::ToggleClipping => self.recorder.toggle(&self.settings)?,
			Message::ClipReceived(clip) => {
				info!("clip recv: {clip}");
				let window = self
					.window_manager
					.get_focused_window()
					.await
					.context("could not get current window")?;
				info!("window: {window:?}");
			}
		}

		return Ok(());
	}
}

#[derive(Debug)]
struct GameChip {
	name: String,
	selected: bool,
}

#[relm4::factory]
impl FactoryComponent for GameChip {
	type Init = String;
	type Input = bool;
	type Output = Message;
	type CommandOutput = ();
	type ParentWidget = gtk::Box;

	view! {
		gtk::ToggleButton {
			set_label: &self.name,
			add_css_class: "pill",

			#[watch]
			set_active: self.selected,

			connect_toggled[sender, index] => move |btn| {
				if btn.is_active() {
					sender.output(Message::GameSelected(index.clone())).unwrap();
				}
			}
		}
	}

	fn init_model(name: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
		return Self { name, selected: false };
	}

	fn update(&mut self, selected: Self::Input, _sender: FactorySender<Self>) {
		self.selected = selected;
	}
}

pub fn run() -> Result<()> {
	glib::log_set_writer_func(|level, fields| {
		fields
			.iter()
			.find(|x| x.key() == "MESSAGE")
			.and_then(|x| x.value_str())
			.inspect(|x| {
				let gtk_span = tracing::span!(tracing::Level::ERROR, "gtk");
				let _enter = gtk_span.enter();
				match level {
					glib::LogLevel::Critical | glib::LogLevel::Error => error!("{x}"),
					glib::LogLevel::Warning => warn!("{x}"),
					glib::LogLevel::Info | glib::LogLevel::Message => debug!("{x}"),
					glib::LogLevel::Debug => trace!("{x}"),
				}
			});

		return glib::LogWriterOutput::Handled;
	});

	RelmApp::new("dev.land.Replayd")
		.with_args(vec![])
		.run_async::<App>("Replayd".to_string());

	return Ok(());
}
