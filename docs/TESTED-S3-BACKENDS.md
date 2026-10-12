# Tested S3 backends

Evidence from the s4-showcase object-store eval (2026-10-12) and this repo’s reference Compose.
Ferrum talks path-style SigV4 S3 (`FERRUM_STORAGE__BACKEND=s3`). It does **not** create buckets.

License lines are taken from each project’s own repository (not Docker Hub labels).

| Backend | Version / pin | License (upstream) | Ferrum S3 probe | Role in Ferrum |
|---------|---------------|--------------------|-----------------|----------------|
| **SeaweedFS** | `chrislusf/seaweedfs:3.80@sha256:1055999e08eed1789b0ae45d235126e4495e23d3fb9d6396293fd42539b1ae6a` | **Apache-2.0** ([seaweedfs/seaweedfs](https://github.com/seaweedfs/seaweedfs)) | 16/16 PASS | **Reference** Compose / offline bundle / Helm local values |
| **Garage** | `dxflrs/garage:v2.0.0@sha256:15b40e0dddd2e611aa746ff6f7c3bfe9f22735e4a2cc29e0abd89c268e9b79d9` | **AGPL-3.0** ([deuxfleurs-org/garage](https://github.com/deuxfleurs-org/garage) `LICENSE` / README) | 16/16 PASS | Tested alternative; used by s4-showcase Contabo demo. Offering Garage as a network service triggers AGPL §13 for *Garage*, not Ferrum. |
| **RustFS** | floating `latest` (unsuitable to pin) | Apache-2.0 (claimed) | 16/16 PASS | BYO only |
| **MinIO** | Hub/binary | **AGPL-3.0** | Hub **401** / `dl.min.io` **410** | **Bring your own S3** — not pulled by Ferrum CI or the offline bundle. Point `FERRUM_STORAGE__S3_*` at an operator-supplied endpoint. |

## Reference choice

**SeaweedFS 3.80** is the default in `deploy/docker-compose.yml` because it is maintained on Docker Hub, Apache-2.0, and auto-creates buckets on Put (init can upload without a separate CreateBucket step). Garage remains fully usable as BYO/alternative when operators accept AGPL for the storage daemon.

## BYO MinIO / other S3

Set on the gateway (and matching credentials for any seed tooling):

```text
FERRUM_STORAGE__BACKEND=s3
FERRUM_STORAGE__S3_ENDPOINT=https://s3.example.org
FERRUM_STORAGE__S3_REGION=…
FERRUM_STORAGE__S3_BUCKET=…
FERRUM_STORAGE__S3_ACCESS_KEY_ID=…
FERRUM_STORAGE__S3_SECRET_ACCESS_KEY=…
```

Do not rely on Ferrum to pull or redistribute MinIO images or binaries.

## Ops exercised

PutObject, multipart (create/upload/complete/abort), GetObject, HeadObject, DeleteObject, path-style, SigV4. DRS `/stream` Range is applied after a full GetObject in Ferrum. Presigned GET needs the `s3_signed` feature (not in published `full-infra` default).
