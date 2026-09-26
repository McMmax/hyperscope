//! Hyperscope Native Profiler Provider Routing & Spark Deprecation.
//!
//! Invariants:
//! 1. Native requests route to Pyroscope (continuous CPU stack sampling) or Tracy (timeline captures);
//!    Spark returns typed error `ERR_PROVIDER_DEPRECATED_FOR_V3` without launching Java.
//! 2. Historical `spark_profile` records remain readable through compatibility tests.
//! 3. Document future Paper/Spigot adapter; zero JVM dependency in native V3.

use serde::{Deserialize, Serialize};

pub const ERR_PROVIDER_DEPRECATED_FOR_V3: &str = "ERR_PROVIDER_DEPRECATED_FOR_V3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfilerKind {
    Pyroscope,
    Tracy,
    Spark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilerRequest {
    pub profiler: ProfilerKind,
    pub server_id: String,
    pub duration_seconds: u32,
    pub sampling_rate_hz: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProfilerResponse {
    PyroscopeStarted {
        session_id: String,
        sampling_rate_hz: u32,
        application_name: String,
    },
    TracyCaptureStarted {
        capture_id: String,
        duration_seconds: u32,
        buffer_size_mb: u32,
    },
    RejectedDeprecated {
        error_code: String,
        message: String,
        recommended_provider: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalSparkProfile {
    pub metadata: SparkMetadata,
    pub thread_dumps: Vec<SparkThreadDump>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparkMetadata {
    pub platform: String,
    pub minecraft_version: String,
    pub timestamp_epoch_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparkThreadDump {
    pub thread_name: String,
    pub stack_trace: Vec<String>,
}

/// Routes a profiler request to the appropriate native engine or rejects legacy Spark.
pub fn route_profiler_request(req: &ProfilerRequest) -> ProfilerResponse {
    match req.profiler {
        ProfilerKind::Pyroscope => ProfilerResponse::PyroscopeStarted {
            session_id: format!("pyro-{}-{}", req.server_id, req.duration_seconds),
            sampling_rate_hz: req.sampling_rate_hz.unwrap_or(100),
            application_name: format!("stomzy-v3-{}", req.server_id),
        },
        ProfilerKind::Tracy => {
            let duration = req.duration_seconds.min(30); // 30s preset cap
            ProfilerResponse::TracyCaptureStarted {
                capture_id: format!("tracy-{}-{}", req.server_id, duration),
                duration_seconds: duration,
                buffer_size_mb: 64,
            }
        }
        ProfilerKind::Spark => ProfilerResponse::RejectedDeprecated {
            error_code: ERR_PROVIDER_DEPRECATED_FOR_V3.to_string(),
            message: "Spark profiling is deprecated for Native V3; use Pyroscope (CPU) or Tracy (Timeline)".to_string(),
            recommended_provider: "Pyroscope".to_string(),
        },
    }
}

/// Parses historical Spark JSON profile records for backward compatibility inspection.
pub fn parse_historical_spark_profile(json_data: &str) -> Result<HistoricalSparkProfile, serde_json::Error> {
    serde_json::from_str(json_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_routing_and_spark_rejection() {
        // 1. Pyroscope routing
        let req_pyro = ProfilerRequest {
            profiler: ProfilerKind::Pyroscope,
            server_id: "lobby-1".into(),
            duration_seconds: 60,
            sampling_rate_hz: Some(100),
        };
        let res_pyro = route_profiler_request(&req_pyro);
        assert!(matches!(res_pyro, ProfilerResponse::PyroscopeStarted { sampling_rate_hz: 100, .. }));

        // 2. Tracy routing
        let req_tracy = ProfilerRequest {
            profiler: ProfilerKind::Tracy,
            server_id: "lobby-1".into(),
            duration_seconds: 45,
            sampling_rate_hz: None,
        };
        let res_tracy = route_profiler_request(&req_tracy);
        assert!(matches!(res_tracy, ProfilerResponse::TracyCaptureStarted { duration_seconds: 30, .. }));

        // 3. Spark rejection
        let req_spark = ProfilerRequest {
            profiler: ProfilerKind::Spark,
            server_id: "lobby-1".into(),
            duration_seconds: 60,
            sampling_rate_hz: None,
        };
        let res_spark = route_profiler_request(&req_spark);
        assert!(matches!(
            res_spark,
            ProfilerResponse::RejectedDeprecated { error_code, .. } if error_code == ERR_PROVIDER_DEPRECATED_FOR_V3
        ));
    }

    #[test]
    fn test_historical_spark_profile_compatibility() {
        let sample_json = r#"{
            "metadata": {
                "platform": "paper",
                "minecraft_version": "1.20.4",
                "timestamp_epoch_ms": 1700000000000
            },
            "thread_dumps": [
                {
                    "thread_name": "Server thread",
                    "stack_trace": [
                        "net.minecraft.server.MinecraftServer.tickChildren(MinecraftServer.java:1234)",
                        "net.minecraft.server.MinecraftServer.tickServer(MinecraftServer.java:987)"
                    ]
                }
            ]
        }"#;

        let parsed = parse_historical_spark_profile(sample_json).unwrap();
        assert_eq!(parsed.metadata.platform, "paper");
        assert_eq!(parsed.thread_dumps.len(), 1);
        assert_eq!(parsed.thread_dumps[0].thread_name, "Server thread");
    }
}
