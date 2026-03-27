use std::{collections::HashMap, fs, path::PathBuf};

use ustr::Ustr;
use v_utils::io::xdg::xdg_data_fallback;

use crate::{
	config::{ArenaConfig, DbBackend},
	rating::EloRating,
};

const APP_NAME: &str = "robot_master";

/// Ratings persistence. Implementations decide storage format.
pub trait RatingDb {
	fn load_ratings(&self) -> HashMap<Ustr, EloRating>;
	fn save_ratings(&self, ratings: &HashMap<Ustr, EloRating>);
}

/// Single-file JSON store for Elo ratings at `$XDG_DATA_HOME/robot_master/ratings.json`.
pub struct JsonRatingDb {
	path: PathBuf,
}

impl JsonRatingDb {
	pub fn new() -> Self {
		let dir = PathBuf::from(xdg_data_fallback()).join(APP_NAME);
		fs::create_dir_all(&dir).expect("failed to create XDG data directory");
		Self { path: dir.join("ratings.json") }
	}
}

impl RatingDb for JsonRatingDb {
	fn load_ratings(&self) -> HashMap<Ustr, EloRating> {
		match fs::read_to_string(&self.path) {
			Ok(contents) => {
				let raw: HashMap<String, EloRating> = serde_json::from_str(&contents).expect("corrupt ratings.json");
				raw.into_iter().map(|(k, v)| (Ustr::from(&k), v)).collect()
			}
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
			Err(e) => panic!("failed to read {}: {e}", self.path.display()),
		}
	}

	fn save_ratings(&self, ratings: &HashMap<Ustr, EloRating>) {
		let raw: HashMap<String, &EloRating> = ratings.iter().map(|(k, v)| (k.to_string(), v)).collect();
		let json = serde_json::to_string_pretty(&raw).expect("failed to serialize ratings");
		fs::write(&self.path, json).expect("failed to write ratings.json");
	}
}

/// Construct the appropriate `RatingDb` from config.
pub fn from_config(config: &ArenaConfig) -> Box<dyn RatingDb> {
	match &config.db_backend {
		DbBackend::Json => Box::new(JsonRatingDb::new()),
		#[cfg(feature = "clickhouse")]
		DbBackend::Clickhouse { url } => Box::new(clickhouse_db::ClickhouseDb::new(url)),
		#[cfg(not(feature = "clickhouse"))]
		DbBackend::Clickhouse { .. } => panic!("compiled without `clickhouse` feature — enable it in robot_master_arena/Cargo.toml"),
	}
}

#[cfg(feature = "clickhouse")]
pub mod clickhouse_db {
	use std::collections::HashMap;

	use ::clickhouse::Client;
	use ustr::Ustr;

	use super::RatingDb;
	use crate::{match_::MatchResult, rating::EloRating};

	pub struct ClickhouseDb {
		client: Client,
	}

	impl ClickhouseDb {
		pub fn new(url: &str) -> Self {
			let client = Client::default().with_url(url);
			Self { client }
		}

		/// Initialize tables. Call once on first run.
		pub async fn init_tables(&self) {
			self.client
				.query(
					"CREATE TABLE IF NOT EXISTS matches (
						timestamp DateTime DEFAULT now(),
						p1_id String,
						p2_id String,
						p1_score UInt16,
						p2_score UInt16,
						p1_weak_line UInt8,
						p2_weak_line UInt8,
						moves String
					) ENGINE = MergeTree() ORDER BY timestamp",
				)
				.execute()
				.await
				.expect("failed to create matches table");

			self.client
				.query(
					"CREATE TABLE IF NOT EXISTS ratings (
						player_id String,
						rating Float64,
						games_played UInt32,
						updated_at DateTime DEFAULT now()
					) ENGINE = ReplacingMergeTree(updated_at) ORDER BY player_id",
				)
				.execute()
				.await
				.expect("failed to create ratings table");
		}

		/// Record a match result (move history + scores).
		pub async fn record_match(&self, result: &MatchResult) {
			let moves_json = serde_json::to_string(&result.moves).expect("failed to serialize moves");
			self.client
				.query("INSERT INTO matches (p1_id, p2_id, p1_score, p2_score, p1_weak_line, p2_weak_line, moves) VALUES (?, ?, ?, ?, ?, ?, ?)")
				.bind(result.p1_id.as_str())
				.bind(result.p2_id.as_str())
				.bind(result.p1_score)
				.bind(result.p2_score)
				.bind(result.p1_weak_line as u8)
				.bind(result.p2_weak_line as u8)
				.bind(&moves_json)
				.execute()
				.await
				.expect("failed to insert match");
		}

		/// Query win rate of p1 against p2.
		pub async fn matchup_winrate(&self, p1: &str, p2: &str) -> (u64, u64, u64) {
			let row = self
				.client
				.query(
					"SELECT
						countIf(p1_score > p2_score) AS p1_wins,
						countIf(p1_score < p2_score) AS p2_wins,
						countIf(p1_score = p2_score) AS draws
					FROM matches
					WHERE (p1_id = ? AND p2_id = ?) OR (p1_id = ? AND p2_id = ?)",
				)
				.bind(p1)
				.bind(p2)
				.bind(p2)
				.bind(p1)
				.fetch_one::<(u64, u64, u64)>()
				.await
				.expect("failed to query matchup");
			row
		}
	}

	impl RatingDb for ClickhouseDb {
		fn load_ratings(&self) -> HashMap<Ustr, EloRating> {
			// ClickHouse is async; use a blocking runtime for the sync trait.
			tokio::runtime::Handle::current().block_on(async {
				let rows: Vec<(String, f64, u32)> = self
					.client
					.query("SELECT player_id, rating, games_played FROM ratings FINAL")
					.fetch_all()
					.await
					.expect("failed to load ratings from ClickHouse");
				rows.into_iter().map(|(id, rating, games_played)| (Ustr::from(&id), EloRating { rating, games_played })).collect()
			})
		}

		fn save_ratings(&self, ratings: &HashMap<Ustr, EloRating>) {
			tokio::runtime::Handle::current().block_on(async {
				for (id, elo) in ratings {
					self.client
						.query("INSERT INTO ratings (player_id, rating, games_played) VALUES (?, ?, ?)")
						.bind(id.as_str())
						.bind(elo.rating)
						.bind(elo.games_played)
						.execute()
						.await
						.expect("failed to save rating to ClickHouse");
				}
			})
		}
	}
}
