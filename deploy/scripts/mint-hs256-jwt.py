#!/usr/bin/env python3
"""Mint an HS256 JWT for Ferrum nightly HelixTest (TEST_BEARER)."""
import base64
import hashlib
import hmac
import json
import os
import sys
import time


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def main() -> int:
    secret = os.environ.get("FERRUM_AUTH__JWT_SECRET") or os.environ.get("HELIXTEST_SHARED_SECRET")
    if not secret:
        print("set FERRUM_AUTH__JWT_SECRET or HELIXTEST_SHARED_SECRET", file=sys.stderr)
        return 1
    now = int(time.time())
    header = b64url(json.dumps({"alg": "HS256", "typ": "JWT"}, separators=(",", ":")).encode())
    claims = b64url(
        json.dumps(
            {
                "iss": "https://auth.ga4gh.test",
                "sub": "test-user",
                "aud": "ferrum",
                "exp": now + 3600,
                "iat": now,
                "scope": "wes.read tes.read",
            },
            separators=(",", ":"),
        ).encode()
    )
    signing = f"{header}.{claims}".encode()
    sig = b64url(hmac.new(secret.encode(), signing, hashlib.sha256).digest())
    print(f"{header}.{claims}.{sig}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
