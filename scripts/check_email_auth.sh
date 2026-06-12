#!/usr/bin/env bash
# Check SPF/DKIM DNS records for synapticfour.com outreach mail.
set -euo pipefail

DOMAIN="${FERRUM_EMAIL_DOMAIN:-synapticfour.com}"
WARN=0

warn() {
  echo "WARN: $*" >&2
  WARN=1
}

ok() {
  echo "OK: $*"
}

echo "Checking email authentication DNS for ${DOMAIN}..."

SPF=$(dig +short TXT "$DOMAIN" 2>/dev/null | tr -d '"' || true)
if [[ -z "$SPF" ]]; then
  warn "No TXT records found at ${DOMAIN} (SPF missing)"
else
  if echo "$SPF" | grep -qi 'v=spf1'; then
    if echo "$SPF" | grep -q 'include:spf.privateemail.com'; then
      ok "SPF includes spf.privateemail.com"
    else
      warn "SPF present but missing include:spf.privateemail.com — expected for PrivateEmail"
    fi
    if echo "$SPF" | grep -q '~all'; then
      ok "SPF uses softfail (~all)"
    elif echo "$SPF" | grep -q '\-all'; then
      ok "SPF uses hardfail (-all)"
    else
      warn "SPF record has no explicit all mechanism"
    fi
  else
    warn "No v=spf1 TXT record at apex ${DOMAIN}"
  fi
fi

DKIM_FOUND=0
for selector in default mail privateemail selector1 selector2; do
  HOST="${selector}._domainkey.${DOMAIN}"
  DKIM=$(dig +short TXT "$HOST" 2>/dev/null | tr -d '"' || true)
  if [[ -n "$DKIM" ]]; then
    ok "DKIM selector ${selector} published at ${HOST}"
    DKIM_FOUND=1
  fi
done

if [[ "$DKIM_FOUND" -eq 0 ]]; then
  warn "No DKIM TXT records found for common selectors on ${DOMAIN}"
  echo "      Enable DKIM in PrivateEmail and publish the selector CNAME/TXT records."
fi

if [[ "$WARN" -ne 0 ]]; then
  echo
  echo "Email authentication check completed with warnings."
  echo "See docs/OPERATIONS.md for recommended SPF/DKIM configuration."
  exit 1
fi

echo
echo "Email authentication check passed."
exit 0
