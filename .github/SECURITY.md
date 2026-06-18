# Security Policy

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Please email security@sorobangate.dev with:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested mitigations

We follow a 90-day responsible disclosure policy and will acknowledge receipt within 48 hours.

## Security Model

- The Admin API must never be exposed to the public internet — bind it to a loopback or private interface only
- API keys are stored hashed (Argon2id) — the plaintext is shown only once on creation
- SorobanGate runs as a non-root user and requires no special capabilities except `CAP_NET_BIND_SERVICE` when binding ports < 1024
- All upstream connections use TLS by default
- Request bodies are streamed and never buffered entirely in memory
