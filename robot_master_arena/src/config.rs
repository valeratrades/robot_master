use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ArenaConfig {
	#[serde(default)]
	pub db_backend: DbBackend,
}

#[derive(Clone, Debug, Deserialize, smart_default::SmartDefault)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DbBackend {
	#[default]
	Json,
	Clickhouse {
		#[serde(default = "default_clickhouse_url")]
		url: String,
	},
}

fn default_clickhouse_url() -> String {
	"http://localhost:8123".to_string()
}
