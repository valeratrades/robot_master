use std::{collections::HashMap, fs, path::PathBuf};

use miette::Diagnostic;
use thiserror::Error;
use ustr::Ustr;
use v_utils::io::xdg::xdg_data_fallback;

use crate::{
	config::{ArenaConfig, DbBackend},
	rating::Rating,
};

const APP_NAME: &str = "robot_master";

/// Ratings persistence. Implementations decide storage format.
pub trait RatingDb: Send + Sync {
	fn load_ratings(&self) -> HashMap<Ustr, Rating>;
	fn save_ratings(&self, ratings: &HashMap<Ustr, Rating>);
}
/// Single-file JSON store for Elo ratings at `$XDG_DATA_HOME/robot_master/ratings.json`.
pub struct JsonRatingDb {
	path: PathBuf,
}
/// Construct the appropriate `RatingDb` from config.
pub fn from_config(config: &ArenaConfig) -> Box<dyn RatingDb> {
	match &config.db_backend {
		DbBackend::Json => Box::new(JsonRatingDb::default()),
		#[cfg(feature = "clickhouse")]
		DbBackend::Clickhouse { url } => Box::new(clickhouse_db::ClickhouseDb::new(url)),
		#[cfg(not(feature = "clickhouse"))]
		DbBackend::Clickhouse { .. } => panic!("compiled without `clickhouse` feature - enable it in robot_master_arena/Cargo.toml"),
	}
}
/// In-memory database: ratings are saved and loaded from memory, never persisted to disk.
/// Used by `arena tourney --no-priors` for ephemeral matchups.
pub struct NoopRatingDb(std::sync::Mutex<HashMap<Ustr, Rating>>);
impl Default for NoopRatingDb {
	fn default() -> Self {
		Self(std::sync::Mutex::new(HashMap::new()))
	}
}
#[derive(Debug, Diagnostic, Error, derive_new::new)]
#[error("failed to load ratings from {path}")]
#[diagnostic(help("the ratings file schema may have changed (e.g. Elo → Glicko-2).\ndelete it and start fresh: rm {path}"))]
struct CorruptRatingsDb {
	path: String,
	#[source]
	source: serde_json::Error,
	#[new(value = "std::backtrace::Backtrace::capture()")]
	backtrace: std::backtrace::Backtrace,
}

impl RatingDb for NoopRatingDb {
	fn load_ratings(&self) -> HashMap<Ustr, Rating> {
		self.0.lock().expect("NoopRatingDb poisoned").clone()
	}

	fn save_ratings(&self, ratings: &HashMap<Ustr, Rating>) {
		*self.0.lock().expect("NoopRatingDb poisoned") = ratings.clone();
	}
}

impl<T: RatingDb + ?Sized> RatingDb for &T {
	fn load_ratings(&self) -> HashMap<Ustr, Rating> {
		(**self).load_ratings()
	}

	fn save_ratings(&self, ratings: &HashMap<Ustr, Rating>) {
		(*self).save_ratings(ratings)
	}
}

impl Default for JsonRatingDb {
	fn default() -> Self {
		let dir = PathBuf::from(xdg_data_fallback()).join(APP_NAME);
		fs::create_dir_all(&dir).expect("failed to create XDG data directory");
		Self { path: dir.join("ratings.json") }
	}
}

impl RatingDb for JsonRatingDb {
	fn load_ratings(&self) -> HashMap<Ustr, Rating> {
		match fs::read_to_string(&self.path) {
			Ok(contents) => {
				let raw: HashMap<String, Rating> = serde_json::from_str(&contents).unwrap_or_else(|e| {
					let report: miette::Report = CorruptRatingsDb::new(self.path.display().to_string(), e).into();
					panic!("{report:?}");
				});
				raw.into_iter().map(|(k, v)| (Ustr::from(&k.to_lowercase()), v)).collect()
			}
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
			Err(e) => panic!("failed to read {}: {e}", self.path.display()),
		}
	}

	fn save_ratings(&self, ratings: &HashMap<Ustr, Rating>) {
		let raw: HashMap<String, &Rating> = ratings.iter().map(|(k, v)| (k.to_string().to_lowercase(), v)).collect();
		let json = serde_json::to_string_pretty(&raw).expect("failed to serialize ratings");
		fs::write(&self.path, json).expect("failed to write ratings.json");
	}
}

#[cfg(feature = "clickhouse")]
pub mod clickhouse_db {
	use std::collections::HashMap;

	use ::clickhouse::Client;
	use ustr::Ustr;

	use super::RatingDb;
	use crate::{match_::MatchResult, rating::Rating};

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
						deviation Float64,
						volatility Float64,
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
		fn load_ratings(&self) -> HashMap<Ustr, Rating> {
			// ClickHouse is async; use a blocking runtime for the sync trait.
			tokio::runtime::Handle::current().block_on(async {
				let rows: Vec<(String, f64, f64, f64)> = self
					.client
					.query("SELECT player_id, rating, deviation, volatility FROM ratings FINAL")
					.fetch_all()
					.await
					.expect("failed to load ratings from ClickHouse");
				rows.into_iter()
					.map(|(id, rating, deviation, volatility)| (Ustr::from(&id.to_lowercase()), Rating { rating, deviation, volatility }))
					.collect()
			})
		}

		fn save_ratings(&self, ratings: &HashMap<Ustr, Rating>) {
			tokio::runtime::Handle::current().block_on(async {
				for (id, r) in ratings {
					let id_lower = id.as_str().to_lowercase();
					self.client
						.query("INSERT INTO ratings (player_id, rating, deviation, volatility) VALUES (?, ?, ?, ?)")
						.bind(&id_lower)
						.bind(r.rating)
						.bind(r.deviation)
						.bind(r.volatility)
						.execute()
						.await
						.expect("failed to save rating to ClickHouse");
				}
			})
		}
	}
}
