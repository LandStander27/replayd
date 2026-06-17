use ksni::TrayMethods;

use super::clip::ClipObject;
use crate::prelude::*;

#[derive(Debug, Clone)]
struct ClipsData {
	store: gio::ListStore,
	factory: gtk::SignalListItemFactory,
	selection: gtk::MultiSelection,
	filter: gtk::CustomFilter,
	sorter: gtk::CustomSorter,
	scores: Arc<RwLock<HashMap<ObjectId, u32>>>,
}

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
	clips_data: ClipsData,
	sort_order: SortOrder,
	user_overrode_sort: bool,
	delete_dialog: AsyncController<ConfirmDialog<adw::ApplicationWindow>>,
	delete_db_dialog: AsyncController<ConfirmDialog<adw::ApplicationWindow>>,
	input_dialog: AsyncController<InputDialog<adw::ApplicationWindow>>,
	properties_dialog: AsyncController<PropertiesDialog<adw::ApplicationWindow>>,
	select_game_dialog: AsyncController<SelectDialog<adw::ApplicationWindow>>,
	selected_game: Option<ObjectId>,
	audio_player: Option<AudioPlayer>,
	main_stack: gtk::Stack,
	search_debounce: Option<glib::SourceId>,
	custom_actions: FactoryVecDeque<CustomActionRow>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::FromRepr)]
pub enum SortOrder {
	Relevance,
	#[default]
	NewestFirst,
	OldestFirst,
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
	LoadGames,
	SearchChanged(String),
	Search(String),
	GameSelected(DynamicIndex),
	GameDeselected,
	ClipReceived(PathBuf),
	SaveClip,
	ToggleClipping,
	PickOutputDir,
	ClearThumbnailCache,
	DeleteDb,
	DeleteDbConfirm,
	SetOutputDir(PathBuf),
	DeleteClips,
	DeleteClipsConfirm,
	RenameClips,
	RenameClipsConfirm(String),
	OpenClipFolder,
	OpenClipId(ObjectId),
	OpenClips,
	OpenClipProperties,
	SelectGame,
	SelectGameConfirm(ObjectId),
	F2Pressed,
	DelPressed,
	EscapePressed,
	SetSortOrder(SortOrder, bool),
	AddCustomAction,
	EditCustomAction(ObjectId),
	EditCustomActionConfirm(CustomAction),
	DeleteCustomAction(ObjectId),
}

relm4::new_action_group!(ClipActionGroup, "clip");
relm4::new_stateless_action!(ClipOpen, ClipActionGroup, "open");
relm4::new_stateless_action!(ClipOpenProperties, ClipActionGroup, "properties");
relm4::new_stateless_action!(ClipOpenFolder, ClipActionGroup, "open-folder");
relm4::new_stateless_action!(ClipRename, ClipActionGroup, "rename");
relm4::new_stateless_action!(ClipSelectGame, ClipActionGroup, "select-game");
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

					gtk::Box {
						set_orientation: gtk::Orientation::Horizontal,
						set_spacing: 8,

						gtk::SearchEntry {
							set_hexpand: true,
							set_placeholder_text: Some("Search clips..."),
							add_css_class: "pill",

							connect_search_changed[sender] => move |entry| sender.input(Message::SearchChanged(entry.text().to_string()))
						},

						gtk::DropDown {
							set_model: Some(&gtk::StringList::new(&["Relevance", "Newest first", "Oldest first"])),

							#[watch]
							#[block_signal(sort_handler)]
							set_selected: app.sort_order as u32,

							set_valign: gtk::Align::Center,

							connect_selected_notify[sender] => move |dd| {
								sender.input(Message::SetSortOrder(SortOrder::from_repr(dd.selected() as usize).unwrap(), true));
							} @sort_handler
						},
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
							set_label: &format!("{}p · {} fps · {}", app.settings.resolution.get_str("display").unwrap(), app.settings.frame_rate.get_str("display").unwrap(), app.settings.codec.get_str("display").unwrap()),
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

				#[local_ref]
				main_stack -> gtk::Stack {
					add_named[Some("loading")] = &gtk::Spinner {
						start: (),
						set_halign: gtk::Align::Center,
						set_valign: gtk::Align::Center,
					},

					add_named[Some("empty-library")] = &adw::StatusPage {
						set_title: "No clips yet",
						set_description: Some("Enable the replay buffer and save your first clip!"),
						set_icon_name: Some("camera-video-symbolic"),
					},

					add_named[Some("empty-search")] = &adw::StatusPage {
						set_title: "No results",
						set_description: Some("Try another search term."),
						set_icon_name: Some("system-search-symbolic"),
					},

					add_named[Some("grid")] = &gtk::ScrolledWindow {
						set_hexpand: true,
						set_vexpand: true,
						set_margin_start: 12,
						set_margin_end: 12,
						set_margin_bottom: 12,

						#[name(clip_grid)]
						gtk::GridView {
							set_model: Some(&clips_data.selection),
							set_factory: Some(&clips_data.factory),
							set_max_columns: 6,
							set_min_columns: 1,
							set_single_click_activate: false,

							connect_activate[sender] => move |_, _| sender.input(Message::OpenClips),

							add_controller = gtk::GestureClick {
								set_button: 1,
								connect_pressed[clip_grid, selection = clips_data.selection] => move |gesture, _, x, y| {
									let picked = clip_grid.pick(x, y, gtk::PickFlags::DEFAULT);

									if picked.as_ref() == Some(clip_grid.upcast_ref()) {
										gesture.set_state(gtk::EventSequenceState::Claimed);
										selection.unselect_all();
									}
								}
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
										gdk::Key::Escape => {
											sender.input(Message::EscapePressed);
											glib::Propagation::Stop
										}
										_ => glib::Propagation::Proceed,
									}
								}
							}
						}
					},

					set_visible_child_name: "loading",
				},

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
								let value = x.value() as u64;
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
							set_model: Some(&gtk::StringList::new(Quality::iter().map(|x| x.get_str("display").unwrap()).collect::<Vec<&str>>().as_slice())),
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
							set_model: Some(&gtk::StringList::new(Codec::iter().map(|x| x.get_str("display").unwrap()).collect::<Vec<&str>>().as_slice())),
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
							set_model: Some(&gtk::StringList::new(Container::iter().map(|x| x.get_str("display").unwrap()).collect::<Vec<&str>>().as_slice())),
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
							set_model: Some(&gtk::StringList::new(Resolution::iter().map(|x| x.get_str("display").unwrap()).collect::<Vec<&str>>().as_slice())),
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
							set_model: Some(&gtk::StringList::new(FrameRate::iter().map(|x| x.get_str("display").unwrap()).collect::<Vec<&str>>().as_slice())),
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
							set_title: "Make a sound feedback whenever you save a clip (requires restart)",

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
					},
					adw::PreferencesGroup {
						set_title: "Custom context menu actions",
						set_description: Some("Add shell commands to run on clips."), // TODO: explain what shell vars you can use

						#[local_ref]
						custom_actions_box -> gtk::ListBox {
							set_selection_mode: gtk::SelectionMode::None,
							add_css_class: "boxed-list",
						},

						#[wrap(Some)]
						set_header_suffix = &gtk::Button {
							set_icon_name: "list-add-symbolic",
							set_valign: gtk::Align::Center,
							add_css_class: "flat",
							connect_clicked => Message::AddCustomAction,
						},
					},
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
				set_visible: !args::args().open_minimized,
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

		if let Err(e) = portals::register().await {
			error!(?e);
			error_dialog.show(format!("{e:#}"));
		}

		let listener = match Listener::bind(
			sender.input_sender().clone(),
			#[cfg(feature = "socket_commands")]
			db.clone(),
		) {
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

		let clips_data = App::setup_clips_factory(sender.input_sender().clone(), db.clone(), &settings);
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
					InputDialogResponse::Confirm(s) => Message::RenameClipsConfirm(s),
					_ => Message::Void,
				}),
			select_game_dialog: SelectDialog::builder()
				.launch(SelectDialogSettings {
					window: root.clone(),
					title: "Set game".to_string(),
					confirm_label: "Set".to_string(),
					cancel_label: "Cancel".to_string(),
				})
				.forward(sender.input_sender(), |msg| match msg {
					SelectDialogResponse::Confirm(id) => Message::SelectGameConfirm(id),
					_ => Message::Void,
				}),
			properties_dialog: PropertiesDialog::builder()
				.launch(PropertiesDialogSettings {
					window: root.clone(),
					close_label: "Close".to_string(),
				})
				.forward(sender.input_sender(), |x| x),
			visible: !args::args().open_minimized,
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
			clips_data: clips_data.clone(),
			sort_order: SortOrder::default(),
			user_overrode_sort: false,
			selected_game: None,
			audio_player,
			main_stack: gtk::Stack::default(),
			search_debounce: None,
			custom_actions: FactoryVecDeque::builder()
				.launch(gtk::ListBox::default())
				.forward(sender.input_sender(), |a| a),
		};

		let main_stack = &app.main_stack;
		let games_box = app.games.widget();
		let custom_actions_box = app.custom_actions.widget();
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

pub(super) fn format_duration(secs: u64, long: bool) -> String {
	let h = secs / 3600;
	let m = (secs % 3600) / 60;
	let s = secs % 60;
	if long {
		if h > 0 {
			return format!("{h}:{m:02}:{s:02}");
		} else if m > 0 {
			return format!("{m:02}:{s:02}");
		} else if s == 1 {
			return "1 second".to_string();
		} else {
			return format!("{s} seconds");
		}
	} else {
		if h > 0 {
			return format!("{h}:{m:02}:{s:02}");
		} else if m > 0 {
			return format!("{m}:{s:02}");
		} else {
			return format!("{s}s");
		}
	}
}

pub(super) fn format_date(timestamp: u64, long: bool) -> String {
	let dt: DateTime<Local> = DateTime::from_timestamp_secs(timestamp as i64)
		.unwrap()
		.with_timezone(&Local);
	if long {
		return dt.format("%b %d, %Y, %-l:%M:%S %p").to_string();
	} else {
		let now = Local::now();
		let today = now.date_naive();
		let clip_day = dt.date_naive();

		if clip_day == today {
			format!("Today at {}", dt.format("%-l:%M %p"))
		} else if clip_day == today.pred_opt().unwrap() {
			format!("Yesterday at {}", dt.format("%-l:%M %p"))
		} else {
			if clip_day.year() == today.year() {
				return dt.format("%b %d at %-l:%M %p").to_string();
			} else {
				return dt.format("%b %d %Y at %-l:%M %p").to_string();
			}
		}
	}
}

fn newest_sort_first(a: &glib::Object, b: &glib::Object) -> gtk::Ordering {
	let a = a.downcast_ref::<ClipObject>().unwrap().clip();
	let b = b.downcast_ref::<ClipObject>().unwrap().clip();
	return b.id.cmp(&a.id).into();
}

impl App {
	fn setup_clips_factory(sender: relm4::Sender<Message>, db: Db, settings: &Settings) -> ClipsData {
		let clips_store = gio::ListStore::new::<ClipObject>();
		let factory = gtk::SignalListItemFactory::new();
		let clips_filter = gtk::CustomFilter::new(|_| true);
		let filter_model = gtk::FilterListModel::new(Some(clips_store.clone()), Some(clips_filter.clone()));
		let clips_sorter = gtk::CustomSorter::new(newest_sort_first);
		let sort_model = gtk::SortListModel::new(Some(filter_model), Some(clips_sorter.clone()));
		let clips_selection = gtk::MultiSelection::new(Some(sort_model));

		factory.connect_setup({
			let clips_selection = clips_selection.clone();
			let actions = settings.custom_actions.clone();
			move |_, item| {
				let item = item.downcast_ref::<gtk::ListItem>().unwrap();

				relm4::view! {
					#[name(bottom_section)]
					gio::Menu {
						append: (Some("Properties"), Some(&ClipOpenProperties::action_name())),
					},

					#[name(danger_section)]
					gio::Menu {
						append: (Some("Delete"), Some(&ClipDelete::action_name())),
					},

					#[name(menu_model)]
					gio::Menu {
						append: (Some("Open"), Some(&ClipOpen::action_name())),
						append: (Some("Rename"), Some(&ClipRename::action_name())),
						append: (Some("Select game"), Some(&ClipSelectGame::action_name())),
						append: (Some("Show in Files"), Some(&ClipOpenFolder::action_name())),
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

						gtk::Overlay {
							add_overlay = &gtk::Label {
								set_halign: gtk::Align::End,
								set_valign: gtk::Align::End,
								set_margin_end: 6,
								set_margin_bottom: 6,
								add_css_class: "caption",
								add_css_class: "duration-overlay",
							},

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
							connect_pressed[clips_selection, popover, item] => move |gesture, _, x, y| {
								if !item.is_selected() {
									clips_selection.select_item(item.position(), true);
								}
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

				if !actions.is_empty() {
					for cmd in &actions {
						menu_model.append(Some(&cmd.name), Some(&format!("clip.custom-{}", cmd.id)));
					}
				}

				menu_model.append_section(None, &danger_section);
				menu_model.append_section(None, &bottom_section);

				item.set_child(Some(&card));
			}
		});

		factory.connect_bind({
			let library = settings.output_dir.clone();
			let actions = settings.custom_actions.clone();
			move |_, item| {
				let item = item.downcast_ref::<gtk::ListItem>().unwrap();
				let clip_obj = item.item().unwrap().downcast::<ClipObject>().unwrap();
				let clip = clip_obj.clip();

				let card = item.child().unwrap().downcast::<gtk::Box>().unwrap();
				card.set_widget_name(&clip.id.to_string());
				let children = card.observe_children();

				let overlay = children
					.item(0)
					.unwrap()
					.downcast::<gtk::Overlay>()
					.unwrap();
				let duration_label = overlay
					.observe_children()
					.item(1) // no clue why this should be 1, expected 0
					.unwrap()
					.downcast::<gtk::Label>()
					.unwrap();

				if let Some(secs) = clip.duration_secs {
					duration_label.set_label(&format_duration(secs, false));
					duration_label.set_visible(true);
				} else {
					duration_label.set_visible(false);
				}

				let stack = overlay.child().unwrap().downcast::<gtk::Stack>().unwrap();
				let title = children.item(1).unwrap().downcast::<gtk::Label>().unwrap();
				let meta = children.item(2).unwrap().downcast::<gtk::Label>().unwrap();

				title.set_label(&clip.title);
				let game = if let Some(game) = clip.game
					&& let Ok(game) = db.get_game(game)
					&& let Some(game) = identifier::get_game(game.game_id)
				{
					Some(game.name.clone())
				} else {
					None
				};

				if let Some(game) = game {
					meta.set_label(&format!("{} · {}", format_date(clip.created, false), game));
				} else {
					meta.set_label(&format_date(clip.created, false));
				}

				relm4::spawn_local({
					let clip = clip.clone();
					let library = library.clone();
					let expected_id = clip.id;
					let card = card.clone();
					async move {
						let thumb_path = tokio::task::spawn_blocking(move || thumbnail::extract(&clip, &library))
							.await
							.ok()
							.and_then(|r| r.ok());

						if card.widget_name() != expected_id.to_string() {
							return; // item was rebound
						}

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
				group.add_action(RelmAction::<ClipSelectGame>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::SelectGame)
				}));
				group.add_action(RelmAction::<ClipOpenProperties>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::OpenClipProperties)
				}));
				group.add_action(RelmAction::<ClipRename>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::RenameClips)
				}));
				group.add_action(RelmAction::<ClipDelete>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::DeleteClips)
				}));
				group.add_action(RelmAction::<ClipOpenFolder>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::OpenClipFolder)
				}));
				group.add_action(RelmAction::<ClipOpen>::new_stateless({
					let sender = sender.clone();
					move |_| sender.emit(Message::OpenClips)
				}));
				let group = group.into_action_group();

				for cmd in &actions {
					let action_name = format!("custom-{}", cmd.id);
					let action = gio::SimpleAction::new(&action_name, None);
					action.connect_activate({
						let cmd = cmd.clone();
						let clip = clip.clone();
						let path = clip.absolute_path(&library);
						let sender = sender.clone();
						move |_, _| {
							let cmd = cmd.clone();
							let clip = clip.clone();
							let path = path.clone();
							let sender = sender.clone();
							tokio::spawn(async move {
								let Ok(mut proc) = Command::new("sh")
									.arg("-c")
									.arg(&cmd.command)
									.env("REPLAYD_CLIP_PATH", path.display().to_string())
									.env("REPLAYD_CLIP_TITLE", &clip.title)
									.stdin(Stdio::null())
									.spawn()
									.context("could not spawn custom action")
									.show_error()
									.emit_error(&sender)
								else {
									return;
								};

								let Ok(status) = proc
									.wait()
									.await
									.context("could not wait for process to end")
									.show_error()
									.emit_error(&sender)
								else {
									return;
								};

								if !status.success() {
									if let Some(code) = status.code() {
										sender.emit(Message::Error(format!("Custom action `{}` returned non-zero exit-code {code}", cmd.name)));
									} else if status.core_dumped() {
										sender.emit(Message::Error(format!("Custom action `{}` core dumped", cmd.name)));
									} else if let Some(signal) = status.signal() {
										if let Ok(signal) = nix::sys::signal::Signal::try_from(signal) {
											sender.emit(Message::Error(format!("Custom action `{}` was terminated from a {signal}", cmd.name)));
										} else {
											sender.emit(Message::Error(format!("Custom action `{}` was terminated", cmd.name)));
										}
									} else if let Some(signal) = status.stopped_signal() {
										if let Ok(signal) = nix::sys::signal::Signal::try_from(signal) {
											sender.emit(Message::Error(format!("Custom action `{}` was stopped from a {signal}", cmd.name)));
										} else {
											sender.emit(Message::Error(format!("Custom action `{}` was stopped", cmd.name)));
										}
									} else {
										sender.emit(Message::Error(format!("Custom action `{}` failed for an unknown reason", cmd.name)));
									}
								}
							});
						}
					});
					group.add_action(&action);
				}

				card.insert_action_group(ClipActionGroup::NAME, Some(&group));
			}
		});

		factory.connect_unbind(|_, item| {
			let item = item.downcast_ref::<gtk::ListItem>().unwrap();
			let card = item.child().unwrap().downcast::<gtk::Box>().unwrap();
			let stack = card
				.observe_children()
				.item(0)
				.unwrap()
				.downcast::<gtk::Overlay>()
				.unwrap()
				.child()
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

		return ClipsData {
			factory,
			filter: clips_filter,
			selection: clips_selection,
			sorter: clips_sorter,
			store: clips_store,
			scores: Arc::default(),
		};
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

	fn apply_sort(&self) {
		let sorter = self.clips_data.sorter.clone();
		let scores = self.clips_data.scores.clone();
		let sort = self.sort_order;
		sorter.set_sort_func(move |a, b| {
			let a = a.downcast_ref::<ClipObject>().unwrap().clip();
			let b = b.downcast_ref::<ClipObject>().unwrap().clip();
			match sort {
				SortOrder::NewestFirst => b.id.cmp(&a.id).into(),
				SortOrder::OldestFirst => a.id.cmp(&b.id).into(),
				SortOrder::Relevance => {
					if let Ok(scores) = scores.read() {
						let a_score = scores.get(&a.id).copied().unwrap_or(0);
						let b_score = scores.get(&b.id).copied().unwrap_or(0);
						return b_score.cmp(&a_score).into();
					} else {
						return b.id.cmp(&a.id).into(); // default NewestFirst
					}
				}
			}
		});
	}

	fn apply_game_filter(&self) {
		let selected_game = self.selected_game;
		self.clips_data.filter.set_filter_func(move |obj| {
			let clip = obj.downcast_ref::<ClipObject>().unwrap().clip();
			match selected_game {
				None => true,
				Some(game) => clip.game.map(|id| game == id).unwrap_or(false),
			}
		});
	}

	fn set_main_stack(&self) -> Result<()> {
		if self
			.db
			.get_num_clips()
			.context("could not get number of clips")?
			== 0
		{
			self.main_stack.set_visible_child_name("empty-library");
		} else {
			self.main_stack.set_visible_child_name("grid");
		}

		return Ok(());
	}

	fn delete_old_games(db: Db, tx: relm4::Sender<Message>) -> Result<()> {
		let clips = db.get_clips()?;
		let mut clips_per_games: HashMap<ObjectId, u64> = HashMap::new();
		let games = db.get_games()?;
		for game in &games {
			clips_per_games.insert(game.id, 0);
		}
		for clip in &clips {
			if let Some(game) = clip.game {
				*clips_per_games.entry(game).or_default() += 1;
			}
		}

		let mut deleted_game = false;
		for game in clips_per_games
			.iter()
			.filter_map(|(&x, &y)| if y == 0 { Some(x) } else { None })
		{
			deleted_game = true;
			db.delete_game(game)?;
		}

		if deleted_game {
			drop(games);
			tx.emit(Message::LoadGames);
		}

		return Ok(());
	}

	fn get_selected_clips(&self) -> Vec<ObjectId> {
		return (0..self.clips_data.selection.n_items())
			.filter_map(|i| {
				if self.clips_data.selection.is_selected(i) {
					Some(
						self.clips_data
							.selection
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
	}

	#[tracing::instrument(skip(self, msg, sender))]
	async fn update(&mut self, msg: Message, sender: AsyncComponentSender<Self>) -> Result<()> {
		let tx = sender.input_sender();
		match msg {
			Message::Void => {}
			Message::AddCustomAction => {
				self.db
					.create_custom_action()
					.context("could not create action")?;
				tx.emit(Message::LoadSettings);
			}
			Message::DeleteCustomAction(id) => {
				self.db.write_settings(|settings| -> Result<()> {
					let index = settings
						.custom_actions
						.iter()
						.position(|x| x.id == id)
						.context("could not find action")?;
					settings.custom_actions.remove(index);

					return Ok(());
				})??;
				tx.emit(Message::LoadSettings);
			}
			Message::EditCustomActionConfirm(action) => {
				self.db.write_settings(|settings| -> Result<()> {
					let old = settings
						.custom_actions
						.iter_mut()
						.find(|x| x.id == action.id)
						.context("could not find action")?;

					*old = action;
					return Ok(());
				})??;
				tx.emit(Message::LoadSettings);
			}
			Message::EditCustomAction(id) => {
				let action = self
					.settings
					.custom_actions
					.iter()
					.find(|x| x.id == id)
					.context("could not find action")?;

				relm4::view! {
					#[name(root)]
					adw::Dialog {
						present: Some(&self.window),
						inline_css: "border-bottom-left-radius: 13px",
						inline_css: "border-bottom-right-radius: 13px",
						set_content_width: 480,

						set_title: "Custom Action",

						#[wrap(Some)]
						set_child = &adw::ToolbarView {
							add_top_bar = &adw::HeaderBar {},

							#[wrap(Some)]
							set_content = &gtk::Box {
								set_orientation: gtk::Orientation::Vertical,

								gtk::Box {
									set_orientation: gtk::Orientation::Vertical,
									set_spacing: 8,
									set_vexpand: true,
									inline_css: "padding: 24px 30px",

									#[name(name)]
									gtk::Entry {
										set_text: &action.name,
										set_placeholder_text: Some("Name"),
										set_activates_default: true,
									},

									#[name(cmd)]
									gtk::Entry {
										set_text: &action.command,
										set_placeholder_text: Some("Shell command"),
										set_activates_default: true,
									},

									gtk::Label {
										set_markup:
r#"

Your command will be ran like this: <tt>sh -c '%COMMAND%'</tt>.
These environment variables are available:
• <tt>REPLAYD_CLIP_PATH</tt>: The full, absolute path to the clip.
• <tt>REPLAYD_CLIP_TITLE</tt>: The title given to the clip.

Note: You must restart Replayd to apply the changes.

"#.trim(),
										set_wrap: true,
										set_halign: gtk::Align::Start,
										set_justify: gtk::Justification::Left,
									},
								},

								gtk::Box {
									set_orientation: gtk::Orientation::Vertical,
									set_vexpand_set: true,
									set_valign: gtk::Align::End,
									gtk::Separator {},

									gtk::Box {
										set_homogeneous: true,
										set_vexpand: true,
										set_valign: gtk::Align::End,

										gtk::Button {
											add_css_class: "flat",
											set_hexpand: true,
											inline_css: "padding: 10px 14px",
											inline_css: "border-radius: 0px",
											inline_css: "border-width: 0px",
											connect_clicked[sender, id = action.id, root] => move |_| {
												sender.input(Message::EditCustomActionConfirm(CustomAction {
													id,
													command: cmd.text().to_string(),
													name: name.text().to_string(),
												}));
												root.close();
											},

											gtk::Label {
												set_label: "Confirm",
												add_css_class: "suggested",
											}
										},

										gtk::Button {
											add_css_class: "flat",
											set_hexpand: true,
											inline_css: "padding: 10px 14px",
											inline_css: "border-radius: 0px",
											inline_css: "border-width: 0px",
											connect_clicked[root] => move |_| _ = root.close(),

											gtk::Label {
												set_label: "Cancel",
												add_css_class: "flat",
											}
										},
									}
								},
							}
						},
					}
				}
			}
			Message::OpenClipProperties => {
				let id = self.get_selected_clips()[0];
				self.clips_data.selection.unselect_all();
				let clip = self.db.get_clip(id)?;
				let thumbnail = thumbnail::extract(&clip, &self.settings.output_dir)?;
				let game = if let Some(id) = clip.game {
					let game = self.db.get_game(id)?;
					Some(
						identifier::get_game(game.game_id)
							.context("unknown game")?
							.name
							.clone(),
					)
				} else {
					None
				};

				let path = clip.absolute_path(&self.settings.output_dir);
				let stat = std::fs::metadata(&path).context("could not stat clip")?;

				let properties = window::dialog::ClipProperties {
					id: clip.id,
					title: clip.title,
					thumbnail: thumbnail.to_string_lossy().to_string(),
					absolute_path: path.to_string_lossy().to_string(),
					game,
					size: stat.size(),
					created: clip.created,
					duration: clip.duration_secs.unwrap_or_default(),
					codec: clip.codec,
					container: clip.container,
					quality: clip.quality,
					resolution: clip.resolution,
					fps: clip.fps,
				};
				info!(?properties);
				self.properties_dialog
					.emit(PropertiesDialogMessage::Show(properties));
			}
			Message::SelectGame => {
				let games = identifier::get_all_games();

				let options = std::iter::once("None".to_string())
					.chain(games.iter().map(|x| x.name.clone()))
					.collect();

				self.select_game_dialog
					.emit(SelectDialogMessage::Show(options));
			}
			Message::SelectGameConfirm(index) => {
				let game_id = if index == 0 {
					None
				} else {
					let game_id = index - 1;
					if let Some(existing) = self.db.get_games()?.iter().find(|x| x.game_id == game_id) {
						Some(existing.id)
					} else {
						let id = self.db.add_game(Game { id: 0, game_id })?;
						tx.emit(Message::LoadGames);
						Some(id)
					}
				};

				for id in self.get_selected_clips() {
					self.db.set_clip_game(id, game_id)?;
				}

				App::delete_old_games(self.db.clone(), tx.clone())?;
				tx.emit(Message::LoadClips);
			}
			Message::SearchChanged(query) => {
				if let Some(id) = self.search_debounce.take() {
					id.remove();
				}

				let tx = tx.clone();
				self.search_debounce = Some(glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
					tx.emit(Message::Search(query));
				}));
			}
			Message::Search(query) => {
				self.search_debounce = None;
				if query.is_empty() {
					self.set_main_stack()?;
					self.user_overrode_sort = false;
					if self.sort_order == SortOrder::Relevance {
						self.sort_order = SortOrder::default();
					}
				} else if !self.user_overrode_sort {
					self.sort_order = SortOrder::Relevance;
				}

				let mut scores = self.clips_data.scores.write().map_err(|x| eyre!("{x}"))?;
				scores.clear();
				self.games.guard().send(0, true);
				if !query.is_empty() {
					let mut searcher = search::Searcher::new();
					for i in 0..self.clips_data.store.n_items() {
						let clip = self
							.clips_data
							.store
							.item(i)
							.unwrap()
							.downcast::<ClipObject>()
							.unwrap()
							.clip();

						let score = searcher.score(&query, &clip.title).unwrap_or(0);
						if score > 0 {
							scores.insert(clip.id, score);
						}
					}

					drop(scores);
					self.clips_data.filter.set_filter_func({
						let scores = self.clips_data.scores.clone();
						move |obj| {
							if let Ok(scores) = scores.read() {
								let clip = obj.downcast_ref::<ClipObject>().unwrap().clip();
								scores.contains_key(&clip.id)
							} else {
								true
							}
						}
					});
					if self.clips_data.selection.n_items() == 0 {
						self.main_stack.set_visible_child_name("empty-search");
					} else {
						self.set_main_stack()?;
					}
				} else {
					drop(scores);
					self.apply_game_filter();
				}

				self.apply_sort();
			}
			Message::SetSortOrder(sort, is_user) => {
				if is_user {
					self.user_overrode_sort = true;
				}
				self.sort_order = sort;
				self.apply_sort();
			}
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
			Message::OpenClipFolder => {
				let id = self.get_selected_clips()[0]; // TODO: add bounds checks
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
			Message::OpenClipId(id) => {
				let clip = self.db.get_clip(id)?;
				let file = gio::File::for_path(clip.absolute_path(&self.settings.output_dir));
				let app = gio::AppInfo::default_for_type("video/mp4", false).context("no default app for video/mp4")?; // the app for mp4s is likely the same for all video formats
				app.launch(&[file], gio::AppLaunchContext::NONE)
					.context("could not open clip")?;
			}
			Message::OpenClips => {
				for id in self.get_selected_clips() {
					tx.emit(Message::OpenClipId(id));
				}
			}
			Message::DelPressed => tx.emit(Message::DeleteClips),
			Message::EscapePressed => {
				self.clips_data.selection.unselect_all();
			}
			Message::DeleteClips => self.delete_dialog.emit(ConfirmDialogMessage::Show),
			Message::DeleteClipsConfirm => {
				let ids = self.get_selected_clips();
				let db = self.db.clone();
				let library = self.settings.output_dir.clone();
				let tx = tx.clone();
				sender.oneshot_command(async move {
					let clips = match db.get_clips() {
						Ok(o) => o,
						Err(e) => {
							error!(?e);
							return Message::Error(format!("{e:#}"));
						}
					};

					for i in (0..clips.len())
						.filter(|i| ids.contains(&clips[*i].id.clone()))
						.rev()
						.collect::<Vec<usize>>()
					{
						let clip = &clips[i];

						if let Err(e) = std::fs::remove_file(clip.absolute_path(&library)).context("could not delete clip file") {
							error!(?e);
							return Message::Error(format!("{e:#}"));
						}

						if let Err(e) = db.delete_clip(clip.id) {
							error!(?e);
							return Message::Error(format!("{e:#}"));
						}
					}

					if let Err(e) = Self::delete_old_games(db, tx) {
						error!(?e);
						return Message::Error(format!("{e:#}"));
					}

					return Message::LoadClips;
				});
			}
			Message::F2Pressed => tx.emit(Message::RenameClips),
			Message::RenameClips => {
				let clips = self.get_selected_clips();
				if clips.len() == 1 {
					self.input_dialog
						.emit(InputDialogMessage::Show(self.db.get_clip(clips[0])?.title));
				} else {
					self.input_dialog
						.emit(InputDialogMessage::Show("New titles".to_string()));
				}
			}
			Message::RenameClipsConfirm(title) => {
				for id in self.get_selected_clips() {
					self.db.rename_clip(id, &title)?;
				}
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
			Message::LoadSettings => {
				self.settings = self.db.read_settings()?;
				let mut guard = self.custom_actions.guard();
				guard.clear();
				for cmd in &self.settings.custom_actions {
					guard.push_back(cmd.clone());
				}
			}
			Message::Error(e) => self.error_dialog.show(e),
			Message::Close => self.visible = false,
			Message::ShowWindow => {
				self.visible = true;
				tx.emit(Message::LoadClips); // update timestamps on clips
			}
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
				relm4::set_global_css(include_str!("../../assets/style.css"));

				info!("loading games");
				if let Err(e) = identifier::get_games()
					.await
					.context("could not get mapped games")
				{
					error!(?e);
					tx.emit(Message::Error(format!("{e:#}")));
				}

				App::delete_old_games(self.db.clone(), tx.clone())?;
				tx.emit(Message::LoadGames);
				info!("loading clips");
				tx.emit(Message::LoadClips);
			}
			Message::LoadClips => {
				self.set_main_stack()?;

				if self
					.db
					.get_num_clips()
					.context("could not get clip amount")?
					== 0
				{
					self.clips_data.store.remove_all();
				} else {
					let clips = self.db.get_clips()?;
					self.clips_data.store.remove_all();
					for clip in clips {
						self.clips_data.store.append(&ClipObject::new(clip));
					}
				}

				self.clips_data.filter.changed(gtk::FilterChange::Different);
			}
			Message::LoadGames => {
				let games = self.db.get_games()?;
				let mut guard = self.games.guard();
				guard.clear();
				guard.push_back((0, "All games".to_string()));
				for game in games {
					guard.push_back((
						game.id,
						identifier::get_game(game.game_id)
							.map(|x| x.name.clone())
							.context("could not get game name")?,
					));
				}
				guard.send(0, true);
			}
			Message::GameDeselected => {
				let guard = self.games.guard();
				if !guard.iter().any(|x| x.selected()) {
					guard.send(0, true);
				}
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

				drop(guard);
				self.apply_game_filter();
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

				let game = if window.fullscreen {
					identifier::identify_game(&window)
						.map(|game_id| {
							let games = self.db.get_games()?;

							if let Some(existing) = games.iter().find(|g| g.game_id == game_id) {
								Ok(existing.id)
							} else {
								let id = self.db.add_game(Game { id: 0, game_id });
								tx.emit(Message::LoadGames);
								return id;
							}
						})
						.transpose()?
				} else {
					None
				};

				let duration_secs = tokio::task::spawn_blocking({
					let path = path.clone();
					move || thumbnail::get_duration(&path)
				})
				.await
				.show_error()
				.inspect(|x| {
					_ = x.as_ref().show_error();
				})
				.map(|x| x.ok())
				.ok()
				.flatten();

				let created = std::time::SystemTime::now()
					.duration_since(std::time::UNIX_EPOCH)
					.unwrap_or_default()
					.as_secs();

				let id = self
					.db
					.save_clip(Clip {
						id: 0,
						title: "Untitled".to_string(),
						path: relative_path,
						codec: self.settings.codec,
						container: self.settings.container,
						fps: self.settings.frame_rate,
						quality: self.settings.quality,
						resolution: self.settings.resolution,
						game,
						duration_secs,
						created,
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
struct CustomActionRow {
	action: CustomAction,
}

#[relm4::factory]
impl FactoryComponent for CustomActionRow {
	type Init = CustomAction;
	type Input = ();
	type Output = Message;
	type CommandOutput = ();
	type ParentWidget = gtk::ListBox;

	view! {
		adw::ActionRow {
			set_title: &self.action.name,
			set_subtitle: &self.action.command,
			set_activatable: true,
			connect_activated[sender, id = self.action.id] => move |_| sender.output(Message::EditCustomAction(id)).unwrap(),

			add_suffix = &gtk::Button {
				set_icon_name: "user-trash-symbolic",
				set_valign: gtk::Align::Center,
				add_css_class: "flat",

				connect_clicked[sender, id = self.action.id] => move |_| sender.output(Message::DeleteCustomAction(id)).unwrap(),
			},
		}
	}

	fn init_model(action: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
		return Self { action };
	}
}

#[derive(Debug)]
struct GameChip {
	name: String,
	game_id: ObjectId,
	selected: Arc<AtomicBool>,
}

impl GameChip {
	pub fn selected(&self) -> bool {
		return self.selected.load(Ordering::Relaxed);
	}
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
			set_active: self.selected.load(Ordering::Relaxed),

			connect_toggled[sender, index, selected = self.selected.clone()] => move |btn| {
				if btn.is_active() {
					selected.store(true, Ordering::Relaxed);
					sender.output(Message::GameSelected(index.clone())).unwrap();
				} else {
					selected.store(false, Ordering::Relaxed);
					sender.output(Message::GameDeselected).unwrap();
				}
			}
		}
	}

	fn init_model((game_id, name): Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
		return Self {
			name,
			game_id,
			selected: Arc::new(AtomicBool::new(false)),
		};
	}

	fn update(&mut self, selected: Self::Input, _sender: FactorySender<Self>) {
		self.selected.store(selected, Ordering::Relaxed);
		// self.selected = selected;
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
		.visible_on_activate(false)
		.with_args(vec![])
		.run_async::<App>("Replayd".to_string());

	return Ok(());
}
