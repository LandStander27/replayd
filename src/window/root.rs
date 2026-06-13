use ksni::TrayMethods;

use super::clip::ClipObject;
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
	clips_store: gio::ListStore,
	clips_selection: gtk::MultiSelection,
	confirm_dialog: AsyncController<ConfirmDialog<adw::ApplicationWindow>>,
	input_dialog: AsyncController<InputDialog<adw::ApplicationWindow>>,
	deleting_clips: Vec<ObjectId>,
	renaming_clip: ObjectId,
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
	LoadClips,
	GamesLoaded(Vec<db::Game>),
	GameSelected(DynamicIndex),
	ClipReceived { path_uri: String, path: PathBuf },
	SaveClip,
	ToggleClipping,
	PickOutputDir,
	SetOutputDir(PathBuf),
	DeleteClips(Vec<ObjectId>),
	DeleteClipsConfirm,
	RenameClip(ObjectId),
	RenameClipConfirm(String),
	OpenClipFolder(ObjectId),
	OpenClip(ObjectId),
	F2Pressed,
	DelPressed,
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
							#[watch]
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
				},

				gtk::ScrolledWindow {
					set_hexpand: true,
					set_vexpand: true,
					set_margin_start: 12,
					set_margin_end: 12,
					set_margin_bottom: 12,

					gtk::GridView {
						set_model: Some(&clips_selection),
						set_factory: Some(&factory),
						set_max_columns: 6,
						set_min_columns: 1,
						set_single_click_activate: false,

						connect_activate[sender, clips_store] => move |_, pos| {
							let obj = clips_store
								.item(pos)
								.unwrap()
								.downcast::<ClipObject>()
								.unwrap();
							sender.input(Message::OpenClip(obj.clip().id));
						},

						add_controller = gtk::EventControllerKey {
							connect_key_pressed[sender] => move |_, key, _, _| {
								match key {
									gdk::Key::F2 => {
										sender.input(Message::F2Pressed);
										glib::Propagation::Stop
									}
									gdk::Key::Delete => {
										sender.input(Message::DelPressed);
										glib::Propagation::Stop
									}
									_ => glib::Propagation::Proceed,
								}
							}
						}
					}
				}
			},
			add_titled_with_icon[Some("settings"), "Settings", "emblem-system-symbolic"] = &gtk::Box {
				adw::PreferencesPage {
					adw::PreferencesGroup {
						set_title: "Replay buffer",
						adw::SwitchRow {
							set_title: "Enable replay buffer",

							#[watch]
							#[block_signal(toggle_handler)]
							set_active: app.recorder.is_active(),

							connect_active_notify => Message::ToggleClipping @toggle_handler,
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
							set_title: "Container",
							set_model: Some(&gtk::StringList::new(&["mp4", "mkv"])),
							set_selected: app.settings.container as u32,
							connect_selected_notify[db, sender] => move |x| {
								let value = x.selected() as usize;
								if let Err(e) = db.write_settings(|s| s.container = Container::from_repr(value).unwrap()) {
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

		let (clips_store, factory, clips_selection) = App::setup_clips_factory(sender.input_sender().clone());
		let app = App {
			window: root.clone(),
			error_dialog,
			confirm_dialog: ConfirmDialog::builder()
				.launch(ConfirmDialogSettings {
					window: root.clone(),
					title: "Are you sure you want to permanently delete this clip?".to_string(),
					message: "If you delete a clip, it is permanently lost.".to_string(),
					accept_label: "Delete".to_string(),
					cancel_label: "Cancel".to_string(),
				})
				.forward(sender.input_sender(), |msg| match msg {
					ConfirmDialogResponse::Confirm => Message::DeleteClipsConfirm,
					_ => Message::Void,
				}),
			input_dialog: InputDialog::builder()
				.launch(InputDialogSettings {
					window: root.clone(),
					title: "Rename clip".to_string(),
					cancel_label: "Cancel".to_string(),
				})
				.forward(sender.input_sender(), |msg| match msg {
					InputDialogResponse::Confirm(s) => Message::RenameClipConfirm(s),
					_ => Message::Void,
				}),
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
			clips_store: clips_store.clone(),
			clips_selection: clips_selection.clone(),
			deleting_clips: Vec::new(),
			renaming_clip: 0,
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

	#[tracing::instrument(skip(self, sender))]
	async fn update_cmd(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>, _window: &adw::ApplicationWindow) {
		if let Err(e) = self.update(msg, sender).await {
			error!(?e);
			self.error_dialog.show(format!("{e:#}"));
		}
	}
}

impl App {
	fn setup_clips_factory(sender: relm4::Sender<Message>) -> (gio::ListStore, gtk::SignalListItemFactory, gtk::MultiSelection) {
		let clips_store = gio::ListStore::new::<ClipObject>();
		let factory = gtk::SignalListItemFactory::new();
		let clips_selection = gtk::MultiSelection::new(Some(clips_store.clone()));

		factory.connect_setup({
			let clips_selection = clips_selection.clone();
			move |_, item| {
				let item = item.downcast_ref::<gtk::ListItem>().unwrap();

				let card = gtk::Box::builder()
					.orientation(gtk::Orientation::Vertical)
					.spacing(0)
					.build();
				card.add_css_class("card");

				let stack = gtk::Stack::new();

				let spinner = gtk::Spinner::builder()
					.spinning(true)
					.halign(gtk::Align::Center)
					.valign(gtk::Align::Center)
					.height_request(120)
					.build();

				let thumb = gtk::Picture::builder()
					.height_request(120)
					.content_fit(gtk::ContentFit::Cover)
					.build();

				stack.add_named(&spinner, Some("loading"));
				stack.add_named(&thumb, Some("thumb"));
				stack.set_visible_child_name("loading");

				let title = gtk::Label::builder()
					.halign(gtk::Align::Start)
					.ellipsize(gtk::pango::EllipsizeMode::End)
					.margin_start(8)
					.margin_end(8)
					.margin_top(6)
					.build();
				title.add_css_class("caption");

				let meta = gtk::Label::builder()
					.halign(gtk::Align::Start)
					.margin_start(8)
					.margin_end(8)
					.margin_bottom(8)
					.build();
				meta.add_css_class("caption");
				meta.add_css_class("dim-label");

				card.append(&stack);
				card.append(&title);
				card.append(&meta);

				let menu_model = gio::Menu::new();
				menu_model.append(Some("Open"), Some("clip.open"));
				menu_model.append(Some("Rename"), Some("clip.rename"));
				menu_model.append(Some("Show in Files"), Some("clip.open-folder"));

				let danger_section = gio::Menu::new();
				danger_section.append(Some("Delete"), Some("clip.delete"));
				menu_model.append_section(None, &danger_section);

				let popover = gtk::PopoverMenu::from_model(Some(&menu_model));
				popover.set_parent(&card);
				popover.set_has_arrow(false);

				let gesture = gtk::GestureClick::new();
				gesture.set_button(3);
				gesture.connect_pressed({
					let clips_selection = clips_selection.clone();
					let popover = popover.clone();
					move |gesture, _, x, y| {
						clips_selection.unselect_all();
						gesture.set_state(gtk::EventSequenceState::Claimed);
						popover.set_halign(gtk::Align::Start);
						popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 0, 0)));
						popover.popup();
					}
				});
				card.add_controller(gesture);

				item.set_child(Some(&card));
			}
		});

		factory.connect_bind({
			// let clips_selection = clips_selection.clone();
			move |_, item| {
				let item = item.downcast_ref::<gtk::ListItem>().unwrap();
				let clip_obj = item.item().unwrap().downcast::<ClipObject>().unwrap();
				let clip = clip_obj.clip();

				let card = item.child().unwrap().downcast::<gtk::Box>().unwrap();
				let children = card.observe_children();

				let stack = children.item(0).unwrap().downcast::<gtk::Stack>().unwrap();
				let title = children.item(1).unwrap().downcast::<gtk::Label>().unwrap();
				let meta = children.item(2).unwrap().downcast::<gtk::Label>().unwrap();

				title.set_label(&clip.title);
				meta.set_label("Unknown game"); // TODO: game resolving

				let path = std::path::PathBuf::from(&clip.path_display);
				relm4::spawn_local(async move {
					let thumb_path = tokio::task::spawn_blocking(move || crate::thumbnail::extract(&path))
						.await
						.ok()
						.and_then(|r| r.ok());

					if let Some(thumb_path) = thumb_path {
						let thumb = stack
							.child_by_name("thumb")
							.unwrap()
							.downcast::<gtk::Picture>()
							.unwrap();
						thumb.set_filename(Some(&thumb_path));
						stack.set_visible_child_name("thumb");
					} else {
						let spinner = stack
							.child_by_name("loading")
							.unwrap()
							.downcast::<gtk::Spinner>()
							.unwrap();
						spinner.set_spinning(false);
					}
				});

				let action_group = gio::SimpleActionGroup::new();

				let rename = gio::SimpleAction::new("rename", None);
				rename.connect_activate({
					let sender = sender.clone();
					let clip = clip.clone();
					move |_, _| sender.emit(Message::RenameClip(clip.id))
				});

				let delete = gio::SimpleAction::new("delete", None);
				delete.connect_activate({
					let sender = sender.clone();
					let id = clip.id;
					// let clips_selection = clips_selection.clone();
					move |_, _| {
						sender.emit(Message::DeleteClips(vec![id]));
						// let mut selected: Vec<ObjectId> = (0..clips_selection.n_items())
						// 	.filter(|&x| clips_selection.is_selected(x))
						// 	.map(|x| {
						// 		clips_selection
						// 			.item(x)
						// 			.unwrap()
						// 			.downcast::<ClipObject>()
						// 			.unwrap()
						// 			.clip()
						// 			.id
						// 	})
						// 	.collect();

						// if selected.is_empty() {
						// 	sender.emit(Message::DeleteClips(vec![clip.id]));
						// } else {
						// 	if !selected.contains(&clip.id) {
						// 		selected.push(clip.id);
						// 	}

						// 	sender.emit(Message::DeleteClips(selected));
						// }
					}
				});

				let open_folder = gio::SimpleAction::new("open-folder", None);
				open_folder.connect_activate({
					let sender = sender.clone();
					let clip = clip.clone();
					move |_, _| sender.emit(Message::OpenClipFolder(clip.id))
				});

				let open = gio::SimpleAction::new("open", None);
				open.connect_activate({
					let sender = sender.clone();
					let clip = clip.clone();
					move |_, _| sender.emit(Message::OpenClip(clip.id))
				});

				action_group.add_action(&rename);
				action_group.add_action(&delete);
				action_group.add_action(&open_folder);
				action_group.add_action(&open);
				card.insert_action_group("clip", Some(&action_group));
			}
		});

		factory.connect_unbind(|_, item| {
			let item = item.downcast_ref::<gtk::ListItem>().unwrap();
			let card = item.child().unwrap().downcast::<gtk::Box>().unwrap();
			let stack = card
				.observe_children()
				.item(0)
				.unwrap()
				.downcast::<gtk::Stack>()
				.unwrap();
			stack.set_visible_child_name("loading");
			let spinner = stack
				.child_by_name("loading")
				.unwrap()
				.downcast::<gtk::Spinner>()
				.unwrap();
			spinner.set_spinning(true);
		});

		return (clips_store, factory, clips_selection);
	}

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
			Message::OpenClipFolder(id) => {
				let clip = self.db.get_clip(id)?;
				let path = std::path::PathBuf::from(&clip.path_display);
				let file = File::open(path).context("could not open file")?;

				let tx = tx.clone();
				let window = self.window.clone();
				relm4::spawn_local(async move {
					if let Err(e) = portals::open_uri::open_directory(&window, file).await {
						error!(?e);
						tx.emit(Message::Error(format!("{e:#}")));
					}
				});
			}
			Message::OpenClip(id) => {
				let clip = self.db.get_clip(id)?;
				let path = std::path::PathBuf::from(&clip.path_display);
				let file = gio::File::for_path(&path);
				let app = gio::AppInfo::default_for_type("video/mp4", false).context("no default app for video/mp4")?;
				app.launch(&[file], gio::AppLaunchContext::NONE)
					.context("could not open clip")?;
			}
			Message::DelPressed => {
				let ids: Vec<ObjectId> = (0..self.clips_selection.n_items())
					.filter_map(|i| {
						if self.clips_selection.is_selected(i) {
							Some(
								self.clips_selection
									.item(i)
									.unwrap()
									.downcast::<ClipObject>()
									.unwrap()
									.clip()
									.id,
							)
						} else {
							None
						}
					})
					.collect();

				if !ids.is_empty() {
					tx.emit(Message::DeleteClips(ids));
				}
			}
			Message::DeleteClips(clips) => {
				self.deleting_clips = clips;
				self.confirm_dialog.emit(ConfirmDialogMessage::Show);
			}
			Message::DeleteClipsConfirm => {
				let clips = std::mem::take(&mut self.deleting_clips);
				let db = self.db.clone();
				sender.oneshot_command(async move {
					for id in clips {
						let clip = match db.get_clip(id) {
							Ok(o) => o,
							Err(e) => {
								error!(?e);
								return Message::Error(format!("{e:#}"));
							}
						};

						if let Err(e) = std::fs::remove_file(clip.path_display).context("could not delete clip file") {
							error!(?e);
							return Message::Error(format!("{e:#}"));
						}

						if let Err(e) = db.delete_clip(id) {
							error!(?e);
							return Message::Error(format!("{e:#}"));
						}
					}
					return Message::LoadClips;
				});
			}
			Message::F2Pressed => {
				let selected: Vec<u32> = (0..self.clips_selection.n_items())
					.filter(|&i| self.clips_selection.is_selected(i))
					.collect();

				if selected.len() == 1 {
					let obj = self
						.clips_selection
						.item(selected[0])
						.unwrap()
						.downcast::<ClipObject>()
						.unwrap();

					tx.emit(Message::RenameClip(obj.clip().id));
				}
			}
			Message::RenameClip(clip) => {
				self.renaming_clip = clip;
				self.input_dialog
					.emit(InputDialogMessage::Show(self.db.get_clip(clip)?.title));
			}
			Message::RenameClipConfirm(title) => {
				let id = std::mem::take(&mut self.renaming_clip);
				self.db.rename_clip(id, title)?;
				tx.emit(Message::LoadClips);
			}
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
				tx.emit(Message::LoadClips);
			}
			Message::LoadClips => {
				let clips = self.db.get_clips()?;
				self.clips_store.remove_all();
				for clip in clips {
					self.clips_store.append(&ClipObject::new(clip));
				}
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
			Message::SaveClip => self.recorder.clip()?,
			Message::ToggleClipping => self.recorder.toggle(&self.settings)?,
			Message::ClipReceived { path, path_uri } => {
				info!("clip recv: {path:?}");
				let window = self
					.window_manager
					.get_focused_window()
					.await
					.context("could not get current window")?;
				info!("window: {window:?}");

				if window.fullscreen {
					todo!();
				}

				let id = self
					.db
					.save_clip(Clip {
						id: 0,
						title: path
							.file_prefix()
							.context("could not get file prefix")?
							.to_string_lossy()
							.to_string(),
						path_display: path.display().to_string(),
						path_uri,
						game: None,
					})
					.context("could not save clip")?;

				info!("clip id: {id}");
				tx.emit(Message::LoadClips);
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
