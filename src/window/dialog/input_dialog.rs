use crate::prelude::*;

#[derive(Debug)]
pub enum InputDialogMessage {
	Show(String),
	Hide,
	Response(InputDialogResponse),
}

#[derive(Debug)]
pub enum InputDialogResponse {
	Confirm(String),
	Cancel,
}

#[derive(Debug)]
pub struct InputDialogSettings<T: IsA<gtk::Widget>> {
	pub window: T,
	pub title: String,
	pub cancel_label: String,
}

#[derive(Debug, Clone)]
pub struct InputDialog<T: IsA<gtk::Widget>> {
	window: T,
	root: adw::Dialog,
	initial: String,
}

impl<T: IsA<gtk::Widget>> InputDialog<T> {
	fn new(window: T, root: adw::Dialog) -> Self {
		return Self {
			window,
			root,
			initial: String::new(),
		};
	}
}

#[relm4::component(pub async)]
impl<T: IsA<gtk::Widget>> SimpleAsyncComponent for InputDialog<T> {
	type Init = InputDialogSettings<T>;
	type Input = InputDialogMessage;
	type Output = InputDialogResponse;

	view! {
		#[name = "root"]
		adw::Dialog {
			inline_css: "border-bottom-left-radius: 13px",
			inline_css: "border-bottom-right-radius: 13px",

			#[wrap(Some)]
			set_child = &gtk::Box {
				set_orientation: gtk::Orientation::Vertical,

				gtk::Box {
					set_orientation: gtk::Orientation::Vertical,
					set_spacing: 8,
					set_vexpand: true,
					inline_css: "padding: 24px 30px",

					gtk::Label {
						set_valign: gtk::Align::Start,
						set_justify: gtk::Justification::Center,
						add_css_class: "title-2",
						set_wrap: true,
						set_max_width_chars: 50,
						set_label: &settings.title,
					},

					gtk::Entry {
						#[watch]
						set_text: &this.initial,
						set_vexpand: true,
						set_valign: gtk::Align::Fill,
						connect_activate[sender] => move |entry| {
							sender.input(InputDialogMessage::Response(InputDialogResponse::Confirm(entry.text().to_string())));
						}
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
							connect_clicked => InputDialogMessage::Response(InputDialogResponse::Cancel),

							gtk::Label {
								set_label: &settings.cancel_label,
								add_css_class: "flat",
							}
						},
					}
				}
			}
		}
	}

	async fn init(settings: Self::Init, root: Self::Root, sender: AsyncComponentSender<Self>) -> AsyncComponentParts<Self> {
		let this = InputDialog::new(settings.window, root.clone());
		let widgets = view_output!();

		return AsyncComponentParts { model: this, widgets };
	}

	async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
		match msg {
			InputDialogMessage::Show(initial) => {
				self.initial = initial;
				self.root.present(Some(&self.window));
			}
			InputDialogMessage::Hide => {
				self.root.close();
			}
			InputDialogMessage::Response(response) => {
				self.root.close();
				sender.output(response).unwrap();
			}
		}
	}
}
