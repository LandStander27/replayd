use std::boxed::Box;
use std::sync::LazyLock;

use image::GenericImageView;
use relm4::Sender;

use crate::prelude::*;

#[derive(Debug)]
pub struct Tray {
	tx: Sender<Message>,
}

impl ksni::Tray for Tray {
	fn id(&self) -> String {
		return "dev.land.Replayd".to_string();
	}

	fn icon_pixmap(&self) -> Vec<ksni::Icon> {
		static ICON: LazyLock<ksni::Icon> = LazyLock::new(|| {
			let img = image::load_from_memory_with_format(include_bytes!("../../assets/icon.png"), image::ImageFormat::Png).expect("valid image");
			let (width, height) = img.dimensions();
			let mut data = img.into_rgba8().into_vec();
			assert_eq!(data.len() % 4, 0);
			for pixel in data.chunks_exact_mut(4) {
				pixel.rotate_right(1) // rgba to argb
			}
			ksni::Icon {
				width: width as i32,
				height: height as i32,
				data,
			}
		});

		vec![ICON.clone()]
	}

	fn title(&self) -> String {
		return "Replayd".to_string();
	}

	fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
		use ksni::menu::*;
		return vec![
			StandardItem {
				label: "Exit".to_string(),
				icon_name: "application-exit".to_string(),
				activate: Box::new(|this: &mut Self| {
					this.tx.send(Message::Exit).expect("main thread died");
				}),
				..Default::default()
			}
			.into(),
		];
	}

	fn activate(&mut self, _x: i32, _y: i32) {
		self.tx.emit(Message::ShowWindow);
	}
}

impl Tray {
	pub fn new(tx: Sender<Message>) -> Self {
		let tray = Tray { tx };
		return tray;
	}
}
