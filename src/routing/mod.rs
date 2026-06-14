use std::collections::HashMap;

#[derive(Clone)]
pub struct Router {
    rules: HashMap<String, String>,
    default_pool: String,
}

impl Router {
    pub fn new(config: &crate::config::RoutingConfig) -> Self {
        let mut rules = HashMap::new();
        for rule in &config.rules {
            for method in &rule.methods {
                rules.insert(method.clone(), rule.pool.clone());
            }
        }
        Self {
            rules,
            default_pool: config.default_pool.clone(),
        }
    }

    pub fn route(&self, method: &str) -> &str {
        self.rules
            .get(method)
            .map(|s| s.as_str())
            .unwrap_or(&self.default_pool)
    }
}

pub fn extract_json_rpc_method(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let method = value.get("method")?.as_str()?;
    Some(method.to_string())
}
