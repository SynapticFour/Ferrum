# Ferrum Operations

Operational runbooks for deployments, outreach, and infrastructure checks outside the application codebase.

---

## Email authentication (contact@synapticfour.com)

Pilot outreach and support mail for Ferrum uses **PrivateEmail** (Namecheap / **jellyfish.systems** SMTP infrastructure).

### Current infrastructure

| Item | Value |
|------|--------|
| Sending domain | `synapticfour.com` |
| From address | `contact@synapticfour.com` |
| SMTP provider | PrivateEmail (`mail.privateemail.com`) |
| DNS host | Operator DNS panel (Namecheap or equivalent) |

Some sending IPs may currently produce **SPF softfail** (`~all` or misaligned includes) when third-party relays send on behalf of the domain without an aligned SPF mechanism.

### Recommended SPF record

Publish a single TXT record at the zone apex (`synapticfour.com`):

```txt
v=spf1 include:spf.privateemail.com ~all
```

If you also send through other relays (e.g. transactional SaaS), add their `include:` mechanisms **before** `~all`. Use `-all` only after all legitimate senders are listed and verified.

Verify propagation:

```bash
dig +short TXT synapticfour.com
```

### DKIM

1. In the PrivateEmail / Namecheap control panel, enable **DKIM** for `synapticfour.com`.
2. Publish the provided **CNAME or TXT** selector records (typically `default._domainkey.synapticfour.com`).
3. Confirm with:

```bash
dig +short TXT default._domainkey.synapticfour.com
```

Mail clients should show **SPF pass** and **DKIM pass** for messages from `contact@synapticfour.com`.

### Automated check

Run the repository script before campaigns or after DNS changes:

```bash
./scripts/check_email_auth.sh
```

The script queries public DNS for apex SPF and common DKIM selectors and prints warnings when records are missing or look misconfigured.

---

## Related documentation

- [COMPLIANCE.md](COMPLIANCE.md) — data protection and table security
- [RELEASING.md](../RELEASING.md) — version tags and release artifacts
