use crate::prelude::*;

#[derive(Debug)]
pub enum SelectDialogMessage {
	Show(Vec<String>),
	Hide,
	Response(SelectDialogResponse),
	Confirm,
}

#[derive(Debug)]
pub enum SelectDialogResponse {
	Confirm(u64),
	Cancel,
}

#[derive(Debug)]
pub struct SelectDialogSettings<T: IsA<gtk::Widget>> {
	pub window: T,
	pub title: String,
	pub confirm_label: String,
	pub cancel_label: String,
}

#[derive(Debug, Clone)]
pub struct SelectDialog<T: IsA<gtk::Widget>> {
	window: T,
	root: adw::Dialog,
	dropdown: gtk::DropDown,
}

#[relm4::component(pub async)]
impl<T: IsA<gtk::Widget>> SimpleAsyncComponent for SelectDialog<T> {
	type Init = SelectDialogSettings<T>;
	type Input = SelectDialogMessage;
	type Output = SelectDialogResponse;

	view! {
		#[name(root)]
		adw::Dialog {
			inline_css: "border-bottom-left-radius: 13px",
			inline_css: "border-bottom-right-radius: 13px",
			set_content_width: 360,

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

					#[local_ref]
					dropdown -> gtk::DropDown {
						set_vexpand: true,
						set_valign: gtk::Align::Center,
						set_enable_search: true,
						set_search_match_mode: gtk::StringFilterMatchMode::Substring,
						set_expression: Some(gtk::PropertyExpression::new(
							gtk::StringObject::static_type(),
							gtk::Expression::NONE,
							"string",
						)),
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
							connect_clicked => SelectDialogMessage::Confirm,

							gtk::Label {
								set_label: &settings.confirm_label,
								add_css_class: "flat",
							}
						},
						gtk::Button {
							add_css_class: "flat",
							set_hexpand: true,
							inline_css: "padding: 10px 14px",
							inline_css: "border-radius: 0px",
							inline_css: "border-width: 0px",
							connect_clicked => SelectDialogMessage::Response(SelectDialogResponse::Cancel),

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
		let this = SelectDialog {
			window: settings.window,
			root: root.clone(),
			dropdown: gtk::DropDown::default(),
		};
		let dropdown = this.dropdown.clone();

		let widgets = view_output!();

		return AsyncComponentParts { model: this, widgets };
	}

	async fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
		match msg {
			SelectDialogMessage::Show(options) => {
				let list = gtk::StringList::new(&options.iter().map(|s| s.as_str()).collect::<Vec<_>>());
				self.dropdown.set_model(Some(&list));
				self.dropdown.set_selected(0);
				self.root.present(Some(&self.window));
			}
			SelectDialogMessage::Confirm => sender.input(SelectDialogMessage::Response(SelectDialogResponse::Confirm(self.dropdown.selected() as u64))),
			SelectDialogMessage::Hide => _ = self.root.close(),
			SelectDialogMessage::Response(response) => {
				self.root.close();
				sender.output(response).unwrap();
			}
		}
	}
}
