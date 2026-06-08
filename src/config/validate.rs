use crate::config::Config;

pub fn validate(config: &Config) -> anyhow::Result<()> {
    anyhow::ensure!(
        !config.pools.is_empty(),
        "At least one upstream pool is required"
    );

    for pool in &config.pools {
        anyhow::ensure!(
            !pool.upstreams.is_empty(),
            "Pool '{}' has no upstreams defined",
            pool.name
        );
        for upstream in &pool.upstreams {
            anyhow::ensure!(
                upstream.weight > 0,
                "Upstream '{}' in pool '{}' must have weight > 0",
                upstream.url,
                pool.name
            );
        }
    }

    anyhow::ensure!(
        config.health_check.healthy_threshold > 0,
        "health_check.healthy_threshold must be > 0"
    );
    anyhow::ensure!(
        config.health_check.unhealthy_threshold > 0,
        "health_check.unhealthy_threshold must be > 0"
    );
    anyhow::ensure!(
        !config.health_check.method.is_empty(),
        "health_check.method must not be empty"
    );

    let pool_names: std::collections::HashSet<&str> =
        config.pools.iter().map(|p| p.name.as_str()).collect();

    anyhow::ensure!(
        pool_names.contains(config.routing.default_pool.as_str()),
        "routing.default_pool '{}' does not match any defined pool",
        config.routing.default_pool
    );

    for rule in &config.routing.rules {
        anyhow::ensure!(
            !rule.methods.is_empty(),
            "Routing rule for pool '{}' has no methods defined",
            rule.pool
        );
        anyhow::ensure!(
            pool_names.contains(rule.pool.as_str()),
            "Routing rule references unknown pool '{}'",
            rule.pool
        );
    }

    if config.cache.enabled && config.cache.backend == crate::config::CacheBackend::Redis {
        anyhow::ensure!(
            config.cache.redis_url.is_some(),
            "cache.redis_url is required when cache backend is 'redis'"
        );
    }

    if config.rate_limit.enabled
        && config.rate_limit.store == crate::config::RateLimitStore::Redis
    {
        anyhow::ensure!(
            config.cache.redis_url.is_some(),
            "cache.redis_url is required when rate_limit store is 'redis'"
        );
    }

    if config.auth.enabled && config.auth.key_store == crate::config::KeyStore::Redis {
        anyhow::ensure!(
            config.cache.redis_url.is_some(),
            "cache.redis_url is required when auth key_store is 'redis'"
        );
    }

    Ok(())
}
