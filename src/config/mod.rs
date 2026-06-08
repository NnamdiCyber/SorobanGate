use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

pub mod validate;

// ── Enums ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Pretty,
}

impl Default for LogFormat {
    fn default() -> Self {
        Self::Json
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadBalancingAlgorithm {
    Wrr,
    Lc,
    Random,
}

impl Default for LoadBalancingAlgorithm {
    fn default() -> Self {
        Self::Wrr
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitStore {
    Memory,
    Redis,
}

impl Default for RateLimitStore {
    fn default() -> Self {
        Self::Memory
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyStore {
    Sqlite,
    Redis,
}

impl Default for KeyStore {
    fn default() -> Self {
        Self::Sqlite
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheBackend {
    Memory,
    Redis,
}

impl Default for CacheBackend {
    fn default() -> Self {
        Self::Memory
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TlsVersion {
    #[serde(rename = "tls1.2")]
    Tls12,
    #[serde(rename = "tls1.3")]
    Tls13,
}

impl Default for TlsVersion {
    fn default() -> Self {
        Self::Tls12
    }
}

// ── Default value helpers ──

fn default_bind() -> SocketAddr {
    "0.0.0.0:8080".parse().unwrap()
}

fn default_admin_bind() -> SocketAddr {
    "127.0.0.1:9000".parse().unwrap()
}

fn default_metrics_bind() -> SocketAddr {
    "0.0.0.0:9090".parse().unwrap()
}

const fn default_request_timeout_ms() -> u64 {
    30_000
}

const fn default_max_connections() -> u32 {
    50_000
}

const fn default_worker_threads() -> usize {
    0
}

const fn default_true() -> bool {
    true
}

const fn default_weight() -> u32 {
    1
}

const fn default_health_interval_ms() -> u64 {
    5_000
}

const fn default_health_timeout_ms() -> u64 {
    2_000
}

const fn default_healthy_threshold() -> u32 {
    2
}

const fn default_unhealthy_threshold() -> u32 {
    3
}

fn default_health_method() -> String {
    "getHealth".to_string()
}

const fn default_warm_up_window_secs() -> u64 {
    30
}

fn default_default_pool() -> String {
    "default".to_string()
}

const fn default_ip_fallback_rps() -> u32 {
    5
}

const fn default_ip_fallback_burst() -> u32 {
    10
}

fn default_db_path() -> PathBuf {
    PathBuf::from("./sorobangate.db")
}

const fn default_max_memory_mb() -> u64 {
    512
}

const fn default_metrics_enabled() -> bool {
    true
}

// ── Config structs ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,
    #[serde(default = "default_admin_bind")]
    pub admin_bind: SocketAddr,
    #[serde(default = "default_metrics_bind")]
    pub metrics_bind: SocketAddr,
    #[serde(default)]
    pub log_level: LogLevel,
    #[serde(default)]
    pub log_format: LogFormat,
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            admin_bind: default_admin_bind(),
            metrics_bind: default_metrics_bind(),
            log_level: LogLevel::default(),
            log_format: LogFormat::default(),
            request_timeout_ms: default_request_timeout_ms(),
            max_connections: default_max_connections(),
            worker_threads: default_worker_threads(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub cert_file: String,
    #[serde(default)]
    pub key_file: String,
    #[serde(default)]
    pub min_version: TlsVersion,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cert_file: String::new(),
            key_file: String::new(),
            min_version: TlsVersion::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamConfig {
    pub url: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
    pub max_connections: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub name: String,
    #[serde(default)]
    pub algorithm: LoadBalancingAlgorithm,
    #[serde(default)]
    pub upstreams: Vec<UpstreamConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    #[serde(default = "default_health_interval_ms")]
    pub interval_ms: u64,
    #[serde(default = "default_health_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_healthy_threshold")]
    pub healthy_threshold: u32,
    #[serde(default = "default_unhealthy_threshold")]
    pub unhealthy_threshold: u32,
    #[serde(default = "default_health_method")]
    pub method: String,
    #[serde(default = "default_warm_up_window_secs")]
    pub warm_up_window_secs: u64,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval_ms: default_health_interval_ms(),
            timeout_ms: default_health_timeout_ms(),
            healthy_threshold: default_healthy_threshold(),
            unhealthy_threshold: default_unhealthy_threshold(),
            method: default_health_method(),
            warm_up_window_secs: default_warm_up_window_secs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub methods: Vec<String>,
    pub pool: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    #[serde(default = "default_default_pool")]
    pub default_pool: String,
    #[serde(default)]
    pub rules: Vec<RoutingRule>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            default_pool: default_default_pool(),
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub store: RateLimitStore,
    #[serde(default = "default_ip_fallback_rps")]
    pub ip_fallback_rps: u32,
    #[serde(default = "default_ip_fallback_burst")]
    pub ip_fallback_burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            store: RateLimitStore::default(),
            ip_fallback_rps: default_ip_fallback_rps(),
            ip_fallback_burst: default_ip_fallback_burst(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyTier {
    pub name: String,
    pub requests_per_second: u32,
    pub burst: u32,
    pub daily_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub allow_unauthenticated: bool,
    #[serde(default)]
    pub key_store: KeyStore,
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    #[serde(default)]
    pub tiers: Vec<KeyTier>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_unauthenticated: true,
            key_store: KeyStore::default(),
            db_path: default_db_path(),
            tiers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheRule {
    pub methods: Vec<String>,
    #[serde(default)]
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub backend: CacheBackend,
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u64,
    pub redis_url: Option<String>,
    #[serde(default)]
    pub rules: Vec<CacheRule>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: CacheBackend::default(),
            max_memory_mb: default_max_memory_mb(),
            redis_url: None,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
    #[serde(default)]
    pub tracing_enabled: bool,
    pub otlp_endpoint: Option<String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            tracing_enabled: false,
            otlp_endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default)]
    pub pools: Vec<PoolConfig>,
    #[serde(default)]
    pub health_check: HealthCheckConfig,
    #[serde(default)]
    pub routing: RoutingConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {}", path, e))?;
        let config: Config = toml::de::from_str(&contents)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file '{}': {}", path, e))?;
        Ok(config)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        validate::validate(self)
    }
}
