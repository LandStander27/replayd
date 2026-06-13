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
	clips_filter: gtk::CustomFilter,
	selected_game: Option<ObjectId>,
	delete_dialog: AsyncController<ConfirmDialog<adw::ApplicationWindow>>,
	delete_db_dialog: AsyncController<ConfirmDialog<adw::ApplicationWindow>>,
	input_dialog: AsyncController<InputDialog<adw::ApplicationWindow>>,
	deleting_clips: Vec<ObjectId>,
	renaming_clip: ObjectId,
	audio_player: Option<AudioPlayer>,
}

#[derive(Debug)]
pub enum Message {
	Void,
	LoadSettings,
	Close,
	ShowWindow,
	Exit,
	ShowAbout,
	Init,
	Error(String),
	LoadClips,
	GamesLoaded(Vec<db::Game>),
	GameSelected(DynamicIndex),
	ClipReceived(PathBuf),
	SaveClip,
	ToggleClipping,
	PickOutputDir,
	ClearThumbnailCache,
	DeleteDb,
	DeleteDbConfirm,
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

relm4::new_action_group!(ClipActionGroup, "clip");
relm4::new_stateless_action!(ClipOpen, ClipActionGroup, "open");
relm4::new_stateless_action!(ClipOpenFolder, ClipActionGroup, "open-folder");
relm4::new_stateless_action!(ClipRename, ClipActionGroup, "rename");
relm4::new_stateless_action!(ClipDelete, ClipActionGroup, "delete");

relm4::new_action_group!(WindowActionGroup, "app");
relm4::new_stateless_action!(AppQuit, WindowActionGroup, "quit");
relm4::new_stateless_action!(AppAbout, WindowActionGroup, "about");

#[relm4::component(async)]
impl AsyncComponent for App {
	type Init = String;
	type Input = Message;
	type Output = ();
	type CommandOutput = Message;

	view! {
		#[name(stack)]
		&adw::ViewStack {
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
							set_subtitle: "How many seconds to keep in memory",
							set_numeric: true,
							#[wrap(Some)]
							set_adjustment = &gtk::Adjustment {
								set_lower: 30.0,
								set_upper: 300.0,
								set_step_increment: 30.0,
							},

							set_value: app.settings.buffer_length as f64,

							connect_changed[db, sender] => move |x| {
								let value = x.value() as i32;
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
							set_subtitle: &app.settings.output_dir.display().to_string(),

							add_suffix = &gtk::Button {
								set_label: "Change",
								set_valign: gtk::Align::Center,
								add_css_class: "flat",
								connect_clicked => Message::PickOutputDir
							},
						},
						adw::ActionRow {
							set_title: "Clear thumbnail cache",

							set_subtitle: &thumbnail::cache_dir().unwrap_or(PathBuf::from("Unknown")).display().to_string(),

							add_suffix = &gtk::Button {
								set_label: "Clear",
								set_valign: gtk::Align::Center,
								add_css_class: "flat",
								connect_clicked => Message::ClearThumbnailCache
							},
						},
						adw::ActionRow {
							set_title: "Delete database",

							set_subtitle: &db::db_path().unwrap_or(PathBuf::from("Unknown")).display().to_string(),

							add_suffix = &gtk::Button {
								set_label: "Delete",
								set_valign: gtk::Align::Center,
								add_css_class: "flat",
								connect_clicked => Message::DeleteDb
							},
						}
					},
					adw::PreferencesGroup {
						set_title: "Window",
						adw::SwitchRow {
							set_title: "Show end title buttons",

							#[watch]
							set_active: app.settings.show_end_title_buttons,

							connect_active_notify[sender, db] => move |x| {
								let value = x.is_active();
								if let Err(e) = db.write_settings(|s| s.show_end_title_buttons = value) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							},
						},
						adw::SwitchRow {
							set_title: "Send notifications on events",

							#[watch]
							set_active: app.settings.notifications,

							connect_active_notify[sender, db] => move |x| {
								let value = x.is_active();
								if let Err(e) = db.write_settings(|s| s.notifications = value) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							},
						},
						adw::SwitchRow {
							set_title: "Make a sound feedback whenever you save a clip",

							#[watch]
							set_active: app.settings.sound_feedback,

							connect_active_notify[sender, db] => move |x| {
								let value = x.is_active();
								if let Err(e) = db.write_settings(|s| s.sound_feedback = value) {
									error!(?e);
									sender.input(Message::Error(format!("{e:#}")));
								} else {
									sender.input(Message::LoadSettings);
								}
							},
						}
					},
					adw::PreferencesGroup {
						set_title: "Miscellaneous",
						adw::ActionRow {
							set_title: "Reset settings to default",
							add_suffix = &gtk::Button {
								set_label: "Reset",
								set_valign: gtk::Align::Center,
								add_css_class: "flat",
								connect_clicked[sender, db] => move |_| {
									if let Err(e) = db.write_settings(|s| *s = Settings::default()) {
										error!(?e);
										sender.input(Message::Error(format!("{e:#}")));
									} else {
										sender.input(Message::LoadSettings);
									}
								}
							},
						}
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
					#[watch]
					set_show_end_title_buttons: app.settings.show_end_title_buttons,

					#[wrap(Some)]
					set_title_widget = &adw::ViewSwitcher {
						set_stack: Some(&stack),
						set_policy: adw::ViewSwitcherPolicy::Wide,
					},

					pack_end = &gtk::MenuButton {
						#[wrap(Some)]
						set_menu_model = &gio::Menu {
							append: (Some("Quit"), Some(&AppQuit::action_name())),
							append: (Some("About Replayd"), Some(&AppAbout::action_name())),
						},
						set_icon_name: "open-menu-symbolic",
						set_tooltip_text: Some("Main Menu")
					}
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

	fn init_loading_widgets(root: Self::Root) -> Option<relm4::loading_widgets::LoadingWidgets> {
		relm4::view! {
			#[local]
			root {
				set_title: Some("Loading..."),

				#[name(spinner)]
				gtk::Spinner {
					start: (),
					set_halign: gtk::Align::Center,
				}
			}
		}

		return Some(relm4::loading_widgets::LoadingWidgets::new(root, spinner));
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

		let audio_player = if settings.sound_feedback {
			match AudioPlayer::new(include_bytes!("../../assets/clip.mp3")) {
				Ok(o) => Some(o),
				Err(e) => {
					error!(?e);
					error_dialog.show(format!("{e:#}"));
					None
				}
			}
		} else {
			None
		};

		let (clips_store, factory, clips_selection, clips_filter) = App::setup_clips_factory(sender.input_sender().clone(), db.clone(), &settings);
		let app = App {
			window: root.clone(),
			error_dialog,
			delete_dialog: ConfirmDialog::builder()
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
			delete_db_dialog: ConfirmDialog::builder()
				.launch(ConfirmDialogSettings {
					window: root.clone(),
					title: "Are you sure you want to delete the database?".to_string(),
					message: "This will clear the clip index, settings, and more. Your clip files will still be on disk. You must restart the app after doing this."
						.to_string(),
					accept_label: "Delete".to_string(),
					cancel_label: "Cancel".to_string(),
				})
				.forward(sender.input_sender(), |msg| match msg {
					ConfirmDialogResponse::Confirm => Message::DeleteDbConfirm,
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
			clips_filter: clips_filter.clone(),
			selected_game: None,
			deleting_clips: Vec::new(),
			renaming_clip: 0,
			audio_player,
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

	async fn post_view() {
		let mut group = RelmActionGroup::<WindowActionGroup>::new();
		group.add_action(RelmAction::<AppQuit>::new_stateless({
			let sender = sender.clone();
			move |_| sender.input(Message::Exit)
		}));
		group.add_action(RelmAction::<AppAbout>::new_stateless({
			let sender = sender.clone();
			move |_| sender.input(Message::ShowAbout)
		}));
		group.register_for_main_application();
	}

	async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>, _window: &adw::ApplicationWindow) {
		if let Err(e) = self.update(msg, sender).await {
			error!(?e);
			self.error_dialog.show(format!("{e:#}"));
		}
	}

	async fn update_cmd(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>, _window: &adw::ApplicationWindow) {
		if let Err(e) = self.update(msg, sender).await {
			error!(?e);
			self.error_dialog.show(format!("{e:#}"));
		}
	}
}

impl App {
	fn setup_clips_factory(
		sender: relm4::Sender<Message>,
		db: Db,
		settings: &Settings,
	) -> (gio::ListStore, gtk::SignalListItemFactory, gtk::MultiSelection, gtk::CustomFilter) {
		let clips_store = gio::ListStore::new::<ClipObject>();
		let factory = gtk::SignalListItemFactory::new();
		let clips_filter = gtk::CustomFilter::new(|_| true);
		let filter_model = gtk::FilterListModel::new(Some(clips_store.clone()), Some(clips_filter.clone()));
		let clips_selection = gtk::MultiSelection::new(Some(filter_model));

		factory.connect_setup({
			let clips_selection = clips_selection.clone();
			move |_, item| {
				let item = item.downcast_ref::<gtk::ListItem>().unwrap();

				relm4::view! {
					#[name(danger_section)]
					gio::Menu {
						append: (Some("Delete"), Some(&ClipDelete::action_name())),
					},

					#[name(menu_model)]
					gio::Menu {
						append: (Some("Open"), Some(&ClipOpen::action_name())),
						append: (Some("Rename"), Some(&ClipRename::action_name())),
						append: (Some("Show in Files"), Some(&ClipOpenFolder::action_name())),
						append_section: (None, &danger_section),
					},

					#[name(popover)]
					gtk::PopoverMenu::from_model(Some(&menu_model)) {
						set_has_arrow: false,
					},

					#[name(card)]
					gtk::Box {
						set_orientation: gtk::Orientation::Vertical,
						set_spacing: 0,
						add_css_class: "card",

						gtk::Stack {
							add_named[Some("loading")] = &gtk::Spinner {
								set_spinning: true,
								set_halign: gtk::Align::Center,
								set_valign: gtk::Align::Center,
								set_height_request: 120,
							},
							add_named[Some("thumb")] = &gtk::Picture {
								set_height_request: 120,
								set_content_fit: gtk::ContentFit::Cover
							},
							set_visible_child_name: "loading"
						},

						gtk::Label {
							set_halign: gtk::Align::Start,
							set_ellipsize: gtk::pango::EllipsizeMode::End,
							set_margin_start: 8,
							set_margin_end: 8,
							set_margin_top: 8,
							add_css_class: "caption"
						},

						gtk::Label {
							set_halign: gtk::Align::Start,
							set_margin_start: 8,
							set_margin_end: 8,
							set_margin_bottom: 8,
							add_css_class: "caption",
							add_css_class: "dim-label",
						},

						add_controller = gtk::GestureClick {
							set_button: 3,
							connect_pressed[clips_selection, popover] => move |gesture, _, x, y| {
								clips_selection.unselect_all();
								gesture.set_state(gtk::EventSequenceState::Claimed);
								popover.set_halign(gtk::Align::Start);
								popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 0, 0)));
								popover.popup();
							}
						}
					},

					#[local]
					popover -> gtk::PopoverMenu {
						set_parent: &card,
					}
				}

				item.set_child(Some(&card));
			}
		});

		factory.connect_bind({
			let library = settings.output_dir.clone();
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
				let game = if let Some(game) = clip.game
					&& let Ok(game) = db.get_game(game)
				{
					identifier::identify_game(&game.window_class).unwrap_or(game.window_class)
				} else {
					"Unknown game".to_string()
				};
				meta.set_label(&game); // TODO: game resolving

				relm4::spawn_local({
					let clip = clip.clone();
					let library = library.clone();
					async move {
						let thumb_path = tokio::task::spawn_blocking(move || thumbnail::extract(&clip, &library))
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
					}
				});

				let mut group = RelmActionGroup::<ClipActionGroup>::new();
				group.add_action(RelmAction::<ClipRename>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::RenameClip(clip.id))
				}));
				group.add_action(RelmAction::<ClipDelete>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::DeleteClips(vec![clip.id]))
				}));
				group.add_action(RelmAction::<ClipOpenFolder>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::OpenClipFolder(clip.id))
				}));
				group.add_action(RelmAction::<ClipOpen>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::OpenClip(clip.id))
				}));
				group.register_for_widget(&card);
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

		return (clips_store, factory, clips_selection, clips_filter);
	}

	fn load_displays(&self, widget: &mut adw::ComboRow) -> Result<()> {
		info!("loading displays");
		let displays = gtk::StringList::new(&[]);
		let v = recorder::get_displays().context("could not get displays")?;

		for display in v.iter() {
			displays.append(display);
		}

		widget.set_model(Some(&displays));
		if let Some(initial) = &self.settings.display {
			widget.set_selected(v.iter().position(|x| x == initial).unwrap_or_default() as u32);
		}

		return Ok(());
	}

	#[tracing::instrument(skip(self, msg, sender))]
	async fn update(&mut self, msg: Message, sender: AsyncComponentSender<Self>) -> Result<()> {
		info!("update");
		let tx = sender.input_sender();
		match msg {
			Message::Void => {}
			Message::ShowAbout => {
				relm4::view! {
					adw::AboutDialog {
						set_application_icon: "dev.land.Replayd",
						set_license_type: gtk::License::MitX11,
						set_website: "https://codeberg.org/Land/replayd",
						set_version: version::version,
						set_developer_name: "Sam Jones",
						set_issue_url: "https://codeberg.org/Land/replayd/issues",
						set_application_name: "Replayd",
						present: Some(&self.window)
					}
				}
			}
			Message::DeleteDb => self.delete_db_dialog.emit(ConfirmDialogMessage::Show),
			Message::DeleteDbConfirm => {
				let path = db::db_path().context("could not get db path")?;
				info!("deleting...");
				std::fs::remove_file(&path).context("could not delete db")?;
				info!("deleted database");
				tx.emit(Message::LoadSettings);
				tx.emit(Message::LoadClips);
			}
			Message::ClearThumbnailCache => thumbnail::clear_cache()?,
			Message::OpenClipFolder(id) => {
				let clip = self.db.get_clip(id)?;
				let file = File::open(clip.absolute_path(&self.settings.output_dir)).context("could not open file")?;

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
				let file = gio::File::for_path(clip.absolute_path(&self.settings.output_dir));
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
				self.delete_dialog.emit(ConfirmDialogMessage::Show);
			}
			Message::DeleteClipsConfirm => {
				let clips = std::mem::take(&mut self.deleting_clips);
				let db = self.db.clone();
				let library = self.settings.output_dir.clone();
				sender.oneshot_command(async move {
					for id in clips {
						let clip = match db.get_clip(id) {
							Ok(o) => o,
							Err(e) => {
								error!(?e);
								return Message::Error(format!("{e:#}"));
							}
						};

						if let Err(e) = std::fs::remove_file(clip.absolute_path(&library)).context("could not delete clip file") {
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
					.write_settings(move |settings| settings.output_dir = dir)?;
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
				if let Err(e) = identifier::get_games()
					.await
					.context("could not get mapped games")
				{
					error!(?e);
					tx.emit(Message::Error(format!("{e:#}")));
				}

				tx.emit(Message::GamesLoaded(self.db.get_games()?));
				info!("loading clips");
				tx.emit(Message::LoadClips);
			}
			Message::LoadClips => {
				let clips = self.db.get_clips()?;
				self.clips_store.remove_all();
				for clip in clips {
					self.clips_store.append(&ClipObject::new(clip));
				}

				self.clips_filter.changed(gtk::FilterChange::Different);
			}
			Message::GamesLoaded(games) => {
				let mut guard = self.games.guard();
				guard.clear();
				guard.push_back((0, "All games".to_string()));
				for game in games {
					guard.push_back((game.id, identifier::identify_game(&game.window_class).unwrap_or(game.window_class)));
				}
				guard.send(0, true);
			}
			Message::GameSelected(index) => {
				let guard = self.games.guard();
				for i in 0..guard.len() {
					guard.send(i, i == index.current_index());
				}

				self.selected_game = if index.current_index() == 0 {
					None
				} else {
					guard.get(index.current_index()).map(|chip| chip.game_id)
				};

				let selected_game = self.selected_game;
				self.clips_filter.set_filter_func(move |obj| {
					let clip = obj.downcast_ref::<ClipObject>().unwrap().clip();
					match selected_game {
						None => true,
						Some(game) => clip.game.map(|id| game == id).unwrap_or(false),
					}
				});
			}
			Message::SaveClip => self.recorder.clip()?,
			Message::ToggleClipping => {
				self.recorder.toggle(&self.settings)?;

				if self.settings.notifications {
					relm4::view! {
						#[name(noti)]
						gio::Notification::new("Replayd") {
							set_body: Some(if self.recorder.is_active() { "Clipping enabled." } else { "Clipping disabled." })
						}
					}

					self.window
						.application()
						.unwrap()
						.send_notification(Some("dev.landsj.Replayd"), &noti);
				}
			}
			Message::ClipReceived(path) => {
				if let Some(audio_player) = &self.audio_player
					&& self.settings.sound_feedback
				{
					audio_player.play().context("could not play audio")?;
				}

				if self.settings.notifications {
					relm4::view! {
						#[name(noti)]
						gio::Notification::new("Replayd") {
							set_body: Some("Clip saved!")
						}
					}

					self.window
						.application()
						.unwrap()
						.send_notification(Some("dev.landsj.Replayd"), &noti);
				}

				info!("clip recv: {path:?}");
				let window = self
					.window_manager
					.get_focused_window()
					.await
					.context("could not get current window")?;
				info!("window: {window:?}");

				let relative_path = path
					.strip_prefix(&self.settings.output_dir)
					.context("invalid clip recv")?
					.to_path_buf();

				let game_id = if window.fullscreen {
					let games = self.db.get_games()?;
					if let Some(game) = games.iter().find(|x| x.window_class == window.class) {
						Some(game.id)
					} else {
						Some(self.db.add_game(Game {
							id: 0,
							window_class: window.class,
						})?)
					}
				} else {
					None
				};

				let id = self
					.db
					.save_clip(Clip {
						id: 0,
						title: relative_path
							.file_prefix()
							.context("could not get file prefix")?
							.to_string_lossy()
							.to_string(),
						path: relative_path,
						game: game_id,
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
	game_id: ObjectId,
	selected: bool,
}

#[relm4::factory]
impl FactoryComponent for GameChip {
	type Init = (ObjectId, String);
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

	fn init_model((game_id, name): Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
		return Self { name, game_id, selected: false };
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
