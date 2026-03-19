-- GA4GH Passports & AAI: visa grants (ControlledAccessGrants, etc.) and optional OAuth clients.

CREATE TABLE passport_visa_grants (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_sub        TEXT NOT NULL,
    user_iss        TEXT NOT NULL,
    dataset_id      TEXT NOT NULL,
    visa_type       TEXT NOT NULL,   -- AffiliationAndRole, AcceptedTermsAndPolicies, ResearcherStatus, ControlledAccessGrants, LinkedIdentities
    value           TEXT NOT NULL,
    source          TEXT NOT NULL,
    conditions      JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ
);

CREATE INDEX idx_passport_visa_grants_user ON passport_visa_grants(user_sub, user_iss);
CREATE INDEX idx_passport_visa_grants_dataset ON passport_visa_grants(dataset_id);

CREATE TABLE passport_clients (
    id              TEXT PRIMARY KEY,
    secret_hash     TEXT,
    redirect_uris   TEXT[],
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO passport_clients (id, redirect_uris) VALUES ('ferrum', ARRAY['http://localhost:8080/callback', 'https://localhost:8080/callback']);

CREATE TABLE passport_auth_codes (
    code            TEXT PRIMARY KEY,
    client_id       TEXT NOT NULL REFERENCES passport_clients(id) ON DELETE CASCADE,
    sub             TEXT NOT NULL,
    iss             TEXT NOT NULL,
    scope           TEXT NOT NULL,
    redirect_uri    TEXT NOT NULL,
    expires_at      TIMESTAMPTZ NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_passport_auth_codes_expires ON passport_auth_codes(expires_at);
