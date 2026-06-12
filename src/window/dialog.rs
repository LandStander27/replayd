use crate::prelude::*;

mod error_dialog;
pub use error_dialog::ErrorDialog;

#[relm4::widget_template]
impl WidgetTemplate for DialogTemplate {
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

					#[name = "title"]
					gtk::Label {
						set_valign: gtk::Align::Start,
						set_justify: gtk::Justification::Center,
						add_css_class: "title-2",
						set_wrap: true,
						set_max_width_chars: 20,
					},

					#[name = "message"]
					gtk::Label {
						set_vexpand: true,
						set_valign: gtk::Align::Fill,
						set_justify: gtk::Justification::Center,
						set_wrap: true,
						set_max_width_chars: 40,
					},
				},

				gtk::Box {
					set_orientation: gtk::Orientation::Vertical,
					set_vexpand_set: true,
					set_valign: gtk::Align::End,
					gtk::Separator {},

					#[name = "buttons"]
					gtk::Box {
						set_homogeneous: true,
						set_vexpand: true,
						set_valign: gtk::Align::End,
					}
				}
			}
		}
	}
}
