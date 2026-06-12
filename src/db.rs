use redb::{Database, ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition, WriteTransaction, backends::InMemoryBackend};

use crate::prelude::*;

mod schema;
pub use schema::*;

// const DATABASE_PATH: &str = "data.redb";
const META_TABLE: TableDefinition<&str, u64> = TableDefinition::new("meta");
const CLIPS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("clips");
const GAMES_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("games");
const SETTINGS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("settings");
const CURRENT_VERSION: u64 = 1;

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
		let path = if cfg!(debug_assertions) {
			"./data.redb".into()
		} else {
			dirs::data_dir()
				.context("could not find XDG_DATA_DIR")?
				.join("dev.land.Replayd")
				.join("data.redb")
		};

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
