use redb::{Database, ReadTransaction, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition, WriteTransaction, backends::InMemoryBackend};

use crate::prelude::*;

mod schema;
pub use schema::*;

// const DATABASE_PATH: &str = "data.redb";
const META_TABLE: TableDefinition<&str, u64> = TableDefinition::new("meta");
const CLIPS_TABLE: TableDefinition<ObjectId, &[u8]> = TableDefinition::new("clips");
const GAMES_TABLE: TableDefinition<ObjectId, &[u8]> = TableDefinition::new("games");
const SETTINGS_TABLE: TableDefinition<ObjectId, &[u8]> = TableDefinition::new("settings");
const CURRENT_VERSION: u64 = 1;

pub fn db_path() -> Result<PathBuf> {
	return Ok(if cfg!(debug_assertions) {
		std::env::current_dir().unwrap().join("data.redb")
	} else {
		dirs::data_dir()
			.context("could not find XDG_DATA_DIR")?
			.join("dev.land.Replayd")
			.join("data.redb")
	});
}

#[derive(Clone)]
pub struct Db {
	db: Arc<Database>,
}

impl Db {
	pub fn memory() -> Result<Self> {
		return Ok(Self {
			db: Arc::new(Database::builder().create_with_backend(InMemoryBackend::new())?),
		});
	}

	pub fn open() -> Result<Self> {
		let path = db_path().context("could not get db path")?;

		std::fs::create_dir_all(path.parent().unwrap()).context("could not create directory")?;

		let db = Database::create(path).context("could not open database")?;
		let this = Self { db: Arc::new(db) };
		this.migrate()?;
		return Ok(this);
	}

	pub fn read_settings(&self) -> Result<Settings> {
		let reader = self.reader()?;
		let table = reader.open_table(SETTINGS_TABLE)?;
		let settings = Settings::decode(table.get(&0)?.context("malformed database")?.value())?;
		return Ok(settings);
	}

	pub fn write_settings<F: FnOnce(&mut Settings) -> R, R>(&self, f: F) -> Result<R> {
		return self.write(|writer| {
			let mut table = writer.open_table(SETTINGS_TABLE)?;
			let mut json = table.get_mut(&0)?.context("malformed database")?;
			let mut settings: Settings = json.value().try_into()?;
			let ret = f(&mut settings);
			json.insert(settings.encode()?.as_slice())?;
			return Ok(ret);
		});
	}

	pub fn get_games(&self) -> Result<Vec<Game>> {
		let reader = self.reader()?;
		let table = reader.open_table(GAMES_TABLE)?;
		let v: Result<Vec<Game>> = table
			.iter()?
			.map(|x| -> Result<Game> {
				let (_, x) = x?;
				return Game::decode(x.value());
			})
			.collect();

		return v;
	}

	pub fn get_clip(&self, id: ObjectId) -> Result<Clip> {
		let reader = self.reader()?;
		let table = reader.open_table(CLIPS_TABLE)?;
		let json = table
			.get(id)
			.context("could not read clips table")?
			.context("clip does not exist")?;

		let clip = Clip::decode(json.value())?;
		return Ok(clip);
	}

	pub fn rename_clip(&self, id: ObjectId, title: String) -> Result<()> {
		return self.write(|writer| {
			let mut table = writer.open_table(CLIPS_TABLE)?;
			let mut json = table
				.get_mut(id)
				.context("could not get clip")?
				.context("invalid clip")?;
			let mut clip = Clip::decode(json.value())?;
			clip.title = title;
			json.insert(clip.encode()?.as_slice())
				.context("could not modify clip")?;
			return Ok(());
		});
	}

	pub fn delete_clip(&self, id: ObjectId) -> Result<()> {
		return self.write(|writer| {
			let mut table = writer.open_table(CLIPS_TABLE)?;
			table.remove(id).context("could not delete clip")?;
			return Ok(());
		});
		// let reader = self.reader()?;
		// let table = reader.open_table(CLIPS_TABLE)?;
		// let json = table
		// 	.get(id)
		// 	.context("could not read clips table")?
		// 	.context("clip does not exist")?;

		// let clip = Clip::decode(json.value())?;
	}

	pub fn get_clips(&self) -> Result<Vec<Clip>> {
		let reader = self.reader()?;
		let table = reader.open_table(CLIPS_TABLE)?;
		let v: Result<Vec<Clip>> = table
			.iter()?
			.map(|x| -> Result<Clip> {
				let (_, x) = x?;
				return Clip::decode(x.value());
			})
			.collect();

		return v;
	}

	pub fn save_clip(&self, mut clip: Clip) -> Result<u64> {
		return self.write(|writer| {
			let mut table = writer.open_table(CLIPS_TABLE)?;
			let len = table.len().context("could not get clip amount")?;
			clip.id = len;
			let json = clip.encode()?;
			table
				.insert(len, json.as_slice())
				.context("could not write clip to database")?;
			return Ok(len);
		});
	}

	fn writer(&self) -> Result<WriteTransaction> {
		return self.db.begin_write().context("could not begin write to db");
	}

	fn write<F: FnOnce(&WriteTransaction) -> Result<R>, R>(&self, f: F) -> Result<R> {
		let writer = self.writer()?;
		let ret = f(&writer)?;
		writer.commit().context("could not commit write")?;
		return Ok(ret);
	}

	fn reader(&self) -> Result<ReadTransaction> {
		return self.db.begin_read().context("could not begin read to db");
	}

	fn schema_version(&self) -> Result<u64> {
		let reader = self.reader()?;

		// if meta table doesn't exist yet, version is 0
		return match reader.open_table(META_TABLE) {
			Ok(table) => Ok(table.get("version")?.map(|v| v.value()).unwrap_or(0)),
			Err(_) => Ok(0),
		};
	}

	fn set_schema_version(&self, version: u64) -> Result<()> {
		self.write(|writer| {
			let mut table = writer.open_table(META_TABLE)?;
			table.insert("version", version)?;
			Ok(())
		})
		.context("could not set schema version")?;
		Ok(())
	}

	fn migrate(&self) -> Result<()> {
		let version = self.schema_version()?;

		if version < 1 {
			self.migration_1()?;
		}
		// if version < 2 {
		// 	self.migration_2()?;
		// }

		assert_eq!(self.schema_version().unwrap(), CURRENT_VERSION);

		return Ok(());
	}

	fn migration_1(&self) -> Result<()> {
		self.write(|write| {
			write.open_table(META_TABLE)?;
			write.open_table(CLIPS_TABLE)?;
			write.open_table(GAMES_TABLE)?;
			let mut table = write.open_table(SETTINGS_TABLE)?;
			table.insert(0, Settings::default().encode()?.as_slice())?;
			Ok(())
		})?;
		self.set_schema_version(1)?;

		return Ok(());
	}
}
