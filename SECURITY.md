# Security policy

## Supported versions

| Version | Supported          |
|---------|--------------------|
| Latest major (e.g. 0.x) | ✅ Yes |
| Older majors / minors | ❌ Best effort only |

We recommend always running the latest patch release of the current major version.

---

## Reporting vulnerabilities

We take security seriously. Please report vulnerabilities **privately** to avoid putting users at risk.

1. **Do not** open a public GitHub issue for a security vulnerability.
2. Email the maintainers (see repository contacts or organization page) or use GitHub Security Advisories if available: **Security** tab → **Report a vulnerability**.
3. Include a description, steps to reproduce, and impact. We will acknowledge and work with you on a fix and disclosure timeline.
4. After a fix is released, we may publish an advisory and credit you unless you prefer to remain anonymous.

---

## Security model overview

- **Authentication:** JWT and/or GA4GH Passports; JWKS and optional issuer validation. See [INSTALLATION.md](docs/INSTALLATION.md) and [GA4GH.md](docs/GA4GH.md).
- **Encryption:** All data at rest is encrypted with Crypt4GH; downloads are re-encrypted per requester. See [CRYPT4GH.md](docs/CRYPT4GH.md) for the threat model and invariants.
- **Authorization:** Passport Visa claims and optional role-based checks before granting access to DRS objects, WES runs, and other resources.

---

## Known security considerations for operators

- **Secrets:** Store database URLs, S3 credentials, and Crypt4GH node keys in secret managers or restricted config; avoid committing them to version control.
- **TLS:** Use HTTPS in production; terminate TLS at a reverse proxy (e.g. nginx) or load balancer.
- **Network:** Restrict access to Ferrum and backing services (PostgreSQL, MinIO) to trusted networks where possible.
- **Updates:** Apply security and dependency updates promptly; run `cargo audit` (or similar) as part of your process.

---

*[← Back to README](README.md)*
