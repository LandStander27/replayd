use super::DialogTemplate;
use crate::prelude::*;

#[derive(Debug)]
pub enum ConfirmDialogMessage {
	Show,
	Hide,
	Response(ConfirmDialogResponse),
}

#[derive(Debug)]
pub enum ConfirmDialogResponse {
	Confirm,
	Cancel,
}

#[derive(Debug)]
pub struct ConfirmDialogSettings<T: IsA<gtk::Widget>> {
	pub window: T,
	pub title: String,
	pub message: String,
	pub accept_label: String,
	pub cancel_label: String,
}

#[derive(Debug, Clone)]
pub struct ConfirmDialog<T: IsA<gtk::Widget>> {
	window: T,
	root: adw::Dialog,
}

impl<T: IsA<gtk::Widget>> ConfirmDialog<T> {
	fn new(window: T, root: adw::Dialog) -> Self {
		return Self { window, root };
	}
}

#[relm4::component(pub async)]
impl<T: IsA<gtk::Widget>> SimpleAsyncComponent for ConfirmDialog<T> {
	type Init = ConfirmDialogSettings<T>;
	type Input = ConfirmDialogMessage;
	type Output = ConfirmDialogResponse;

	view! {
		#[root]
		#[template]
		DialogTemplate {
			add_controller = gtk::EventControllerKey {
				connect_key_pressed[sender] => move |_, key, _, _| {
					if key == gdk::Key::Return || key == gdk::Key::KP_Enter {
						sender.input(ConfirmDialogMessage::Response(ConfirmDialogResponse::Confirm));
						glib::Propagation::Stop
					} else {
						glib::Propagation::Proceed
					}
				}
			},

			#[template_child]
			title {
				set_text: &settings.title
			},

			#[template_child]
			message {
				set_text: &settings.message
			},

			#[template_child]
			buttons {
				gtk::Button {
					add_css_class: "destructive-action",
					set_hexpand: true,
					inline_css: "padding: 10px 14px",
					inline_css: "border-radius: 0px",
					inline_css: "border-width: 0px",
					connect_clicked => ConfirmDialogMessage::Response(ConfirmDialogResponse::Confirm),

					gtk::Label {
						set_label: &settings.accept_label,
						add_css_class: "flat",
					}
				},

				gtk::Button {
					add_css_class: "flat",
					set_hexpand: true,
					inline_css: "padding: 10px 14px",
					inline_css: "border-radius: 0px",
					inline_css: "border-width: 0px",
					connect_clicked => ConfirmDialogMessage::Response(ConfirmDialogResponse::Cancel),

					gtk::Label {
						set_label: &settings.cancel_label,
						add_css_class: "flat",
					}
				},
			}
		}
	}

	async fn init(settings: Self::Init, root: Self::Root, sender: AsyncComponentSender<Self>) -> AsyncComponentParts<Self> {
		let this = ConfirmDialog::new(settings.window, root.root.clone());
		let widgets = view_output!();

		return AsyncComponentParts { model: this, widgets };
	}

	async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
		match msg {
			ConfirmDialogMessage::Show => self.root.present(Some(&self.window)),
			ConfirmDialogMessage::Hide => {
				self.root.close();
			}
			ConfirmDialogMessage::Response(response) => {
				self.root.close();
				sender.output(response).unwrap();
			}
		}
	}
}
