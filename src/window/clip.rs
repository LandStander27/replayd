use crate::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::RefCell;

mod imp {
	use super::*;

	#[derive(Default)]
	pub struct ClipObject {
		pub clip: RefCell<Option<Clip>>,
	}

	#[glib::object_subclass]
	impl ObjectSubclass for ClipObject {
		const NAME: &'static str = "ClipObject";
		type Type = super::ClipObject;
	}

	impl ObjectImpl for ClipObject {}
}

glib::wrapper! {
	pub struct ClipObject(ObjectSubclass<imp::ClipObject>);
}

impl ClipObject {
	pub fn new(clip: Clip) -> Self {
		let obj: Self = glib::Object::new();
		*obj.imp().clip.borrow_mut() = Some(clip);
		obj
	}

	pub fn clip(&self) -> Clip {
		self.imp().clip.borrow().clone().unwrap()
	}
}
