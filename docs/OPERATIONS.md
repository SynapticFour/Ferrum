# Operations

This page is the email-authentication check that [CHANGELOG.md](../CHANGELOG.md) and `scripts/check_email_auth.sh` already name. It does not describe how to run the gateway. Gateway operation is in [FIELD-OPS.md](FIELD-OPS.md). An incident is in [INCIDENT_RESPONSE.md](INCIDENT_RESPONSE.md).

## Email authentication DNS

`scripts/check_email_auth.sh` queries DNS for the domain in `FERRUM_EMAIL_DOMAIN`. When that variable is unset, the script uses `synapticfour.com`. It looks for:

- a TXT record at the domain apex that contains `v=spf1`
- that same record containing `include:spf.privateemail.com`
- an `all` mechanism of `~all` or `-all`
- a DKIM TXT record for one of the selectors `default`, `mail`, `privateemail`, `selector1`, or `selector2`, at `<selector>._domainkey.<domain>`

The script prints `OK:` or `WARN:` lines. Any warning exits 1. The script does not change DNS.

This is a check of published DNS for outreach mail. It is not a statement that a Ferrum deployment sends mail.
