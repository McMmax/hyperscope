//! Hyperscope vitals collection — platform-neutral.
//!
//! A *collector* periodically reads a server's live perf and pushes it into the
//! Hyperscope `server_vitals` table by calling the `report_game_vitals` reducer.
//! This crate provides the two reusable pieces:
//!
//!   * [`perf`] — fetch + deserialize a game server's perf JSON endpoint.
//!   * [`report`] — map that perf into the exact positional argument array the
//!     `report_game_vitals` reducer expects.
//!
//! It deliberately speaks the reducer's HTTP **wire format** (a JSON array POSTed
//! to `/v1/database/<db>/call/report_game_vitals`) rather than the SpacetimeDB
//! SDK, so it stays a small, dependency-light building block any host (a daemon,
//! a sidecar, the game server itself) can drive on whatever schedule it likes.
//!
//! The reducer is gated server-side by `hyperscope_settings.enabled`, so a
//! collector may post unconditionally — disabled servers simply drop the write.
//!
//! # Example
//! ```ignore
//! let perf = hyperscope_collector::perf::fetch_game_perf(&client, "http://localhost:26566").await?;
//! let args = hyperscope_collector::report::GameVitalsArgs::from_game_perf("lobby", &perf);
//! client.post(format!("{uri}/v1/database/{db}/call/report_game_vitals"))
//!     .json(&args.to_call_array())
//!     .send().await?;
//! ```

pub mod perf;
pub mod report;

pub use perf::{fetch_game_perf, GamePerf};
pub use report::GameVitalsArgs;
