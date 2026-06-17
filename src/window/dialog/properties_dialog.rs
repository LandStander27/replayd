use crate::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct ClipProperties {
	pub id: ObjectId,
	pub title: String,
	pub thumbnail: String,
	pub absolute_path: String,
	pub game: Option<String>,
	pub size: u64,
	pub created: u64,
	pub duration: u64,
	pub quality: Quality,
	pub resolution: Resolution,
	pub codec: Codec,
	pub container: Container,
	pub fps: FrameRate,
}

#[derive(Debug)]
pub struct PropertiesDialogSettings<T: IsA<gtk::Widget>> {
	pub window: T,
	pub close_label: String,
}

#[derive(Debug)]
pub enum PropertiesDialogMessage {
	Show(ClipProperties),
	CopyClipPath,
	OpenClip,
}

#[derive(Debug, Clone)]
pub struct PropertiesDialog<T: IsA<gtk::Widget>> {
	window: T,
	root: adw::Dialog,
	properties: ClipProperties,
}

#[relm4::component(pub async)]
impl<T: IsA<gtk::Widget>> SimpleAsyncComponent for PropertiesDialog<T> {
	type Init = PropertiesDialogSettings<T>;
	type Input = PropertiesDialogMessage;
	type Output = Message;

	view! {
		#[name(root)]
		adw::Dialog {
			inline_css: "border-bottom-left-radius: 13px",
			inline_css: "border-bottom-right-radius: 13px",
			set_content_width: 650,
			set_content_height: 800,

			#[watch]
			set_title: &this.properties.title,

			#[wrap(Some)]
			set_child = &adw::ToolbarView {
				add_top_bar = &adw::HeaderBar {},

				#[wrap(Some)]
				set_content = &gtk::Box {
					set_orientation: gtk::Orientation::Vertical,

					adw::Clamp {
						set_maximum_size: 400,
						set_unit: adw::LengthUnit::Sp,
						gtk::Overlay {
							#[wrap(Some)]
							set_child = &gtk::Picture {
								set_content_fit: gtk::ContentFit::Contain,

								#[watch]
								set_filename: Some(&this.properties.thumbnail),
							},

							add_overlay = &gtk::Button {
								set_halign: gtk::Align::Center,
								set_valign: gtk::Align::Center,
								add_css_class: "circular",
								add_css_class: "play-overlay",

								set_icon_name: "media-playback-start-symbolic",

								connect_clicked => PropertiesDialogMessage::OpenClip,
							}
						}
					},
					adw::PreferencesPage {
						adw::PreferencesGroup {
							set_title: "General",

							adw::ActionRow {
								set_title: "Title",

								#[watch]
								set_subtitle: &this.properties.title,
							},

							adw::ActionRow {
								set_title: "Game",

								#[watch]
								set_subtitle: this.properties.game.as_deref().unwrap_or("Unknown"),
							},

							adw::ActionRow {
								set_title: "Created",

								#[watch]
								set_subtitle: &crate::window::root::format_date(this.properties.created, true),
							},

							adw::ActionRow {
								set_title: "Duration",

								#[watch]
								set_subtitle: &crate::window::root::format_duration(this.properties.duration, true),
							},
						},

						adw::PreferencesGroup {
							set_title: "File",

							adw::ActionRow {
								set_title: "Location",

								#[watch]
								set_subtitle: &this.properties.absolute_path,

								add_suffix = &gtk::Button {
									set_tooltip: "Copy path",
									set_icon_name: "edit-copy-symbolic",
									add_css_class: "flat",

									connect_clicked => PropertiesDialogMessage::CopyClipPath,
								},
							},
							adw::ActionRow {
								set_title: "Size",

								#[watch]
								set_subtitle: &humansize::format_size(this.properties.size, humansize::WINDOWS).to_lowercase(),
							},
						},

						adw::PreferencesGroup {
							set_title: "Video",

							adw::ActionRow {
								set_title: "Quality",

								#[watch]
								set_subtitle: &this.properties.quality.get_str("display").unwrap(),
							},

							adw::ActionRow {
								set_title: "Resolution",

								#[watch]
								set_subtitle: &this.properties.resolution.get_str("display").unwrap(),
							},

							adw::ActionRow {
								set_title: "Codec",

								#[watch]
								set_subtitle: &this.properties.codec.get_str("display").unwrap(),
							},

							adw::ActionRow {
								set_title: "FPS",

								#[watch]
								set_subtitle: &this.properties.fps.get_str("display").unwrap(),
							},

							adw::ActionRow {
								set_title: "Container",

								#[watch]
								set_subtitle: &this.properties.container.get_str("display").unwrap(),
							},
						}
					}
				}
			}
		}
	}

	async fn init(settings: Self::Init, root: Self::Root, sender: AsyncComponentSender<Self>) -> AsyncComponentParts<Self> {
		let this = PropertiesDialog {
			window: settings.window,
			root: root.clone(),
			properties: ClipProperties::default(),
		};

		let widgets = view_output!();

		return AsyncComponentParts { model: this, widgets };
	}

	async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
		match msg {
			PropertiesDialogMessage::Show(props) => {
				self.properties = props;
				self.root.present(Some(&self.window));
			}
			PropertiesDialogMessage::CopyClipPath => gdk::Display::default()
				.unwrap()
				.clipboard()
				.set_text(&self.properties.absolute_path),
			PropertiesDialogMessage::OpenClip => sender
				.output(Message::OpenClipId(self.properties.id))
				.unwrap(),
		}
	}
}
