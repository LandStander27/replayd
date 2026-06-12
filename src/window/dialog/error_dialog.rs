use super::DialogTemplate;
use crate::prelude::*;

#[derive(Debug)]
pub struct ErrorDialog<T: IsA<gtk::Widget>> {
	controller: AsyncController<ErrorDialogComponent<T>>,
}

impl<T: IsA<gtk::Widget>> ErrorDialog<T> {
	pub fn new(window: T) -> Self {
		return Self {
			controller: ErrorDialogComponent::builder().launch(window).detach(),
		};
	}

	pub fn show(&self, message: impl Into<String>) {
		self.controller
			.emit(ErrorDialogMessage::Show(message.into()));
	}
}

#[derive(Debug)]
enum ErrorDialogMessage {
	Show(String),
	Hide,
}

#[derive(Debug, Clone)]
struct ErrorDialogComponent<T: IsA<gtk::Widget>> {
	window: T,
	root: adw::Dialog,
	message: String,
}

impl<T: IsA<gtk::Widget>> ErrorDialogComponent<T> {
	fn new(window: T, root: adw::Dialog) -> Self {
		return Self {
			window,
			root,
			message: String::new(),
		};
	}
}

#[relm4::component(async)]
impl<T: IsA<gtk::Widget>> SimpleAsyncComponent for ErrorDialogComponent<T> {
	type Init = T;
	type Input = ErrorDialogMessage;
	type Output = ();

	view! {
		#[root]
		#[template]
		DialogTemplate {
			#[template_child]
			title {
				set_text: "An error occurred"
			},

			#[template_child]
			message {
				#[watch]
				set_text: &this.message
			},

			#[template_child]
			buttons {
				gtk::Button {
					add_css_class: "flat",
					set_hexpand: true,
					inline_css: "padding: 10px 14px",
					inline_css: "border-radius: 0px",
					inline_css: "border-width: 0px",
					connect_clicked => ErrorDialogMessage::Hide,

					gtk::Label {
						set_label: "Ok",
						add_css_class: "flat",
					}
				},
			}
		}
	}

	async fn init(window: Self::Init, root: Self::Root, _sender: AsyncComponentSender<Self>) -> AsyncComponentParts<Self> {
		let this = ErrorDialogComponent::new(window, root.root.clone());
		let widgets = view_output!();

		return AsyncComponentParts { model: this, widgets };
	}

	async fn update(&mut self, msg: Self::Input, _sender: AsyncComponentSender<Self>) {
		match msg {
			ErrorDialogMessage::Show(s) => {
				self.message = s;
				self.root.present(Some(&self.window))
			}
			ErrorDialogMessage::Hide => {
				self.root.close();
			}
		}
	}
}
