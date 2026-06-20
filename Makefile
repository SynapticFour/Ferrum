# Ferrum – local demo and dev targets
# Run from repo root.

COMPOSE_FILE := deploy/docker-compose.yml
COMPOSE := docker compose -f $(COMPOSE_FILE)
GA4GH_INFRA_SRC ?= $(abspath ../ga4gh-infra)
export GA4GH_INFRA_SRC
COMPOSE_PILOT := docker compose -f deploy/docker-compose.yml -f deploy/docker-compose.ga4gh-infra.yml -f deploy/docker-compose.pilot.yml
COMPOSE_PILOT_CLOUD := docker compose -f deploy/docker-compose.yml -f deploy/docker-compose.pilot-cloud.yml
COMPOSE_TES := docker compose -f deploy/docker-compose.yml -f deploy/docker-compose.tes.yml
FERRUM_WES_TES_WORK_HOST_PREFIX ?= $(CURDIR)/deploy/.wes-runs
export FERRUM_WES_TES_WORK_HOST_PREFIX
DOCKER_BIN ?= $(shell command -v docker 2>/dev/null || echo /usr/local/bin/docker)
export DOCKER_BIN

.PHONY: help up down destroy demo stop clean clean-all logs pull build rebuild rebuild-gateway edge laptop up-pilot down-pilot up-pilot-cloud down-pilot-cloud up-tes seed-pilot smoke-pilot test-demo test-tes test-tes-full test-pilot test-pilot-cloud test-federated

# Synaptic Four unified local lifecycle: up → down → destroy
help:
	@echo "Ferrum — local lifecycle (Synaptic Four GA4GH stack)"
	@echo ""
	@echo "  make up        Start demo stack (alias: make demo)"
	@echo "  make up-tes    Demo stack + Docker-backed TES (real container runs)"
	@echo "  make seed-pilot  Optional: upload pilot BAM+VCF+ref bundle to MinIO (stack must be running)"
	@echo "  make smoke-pilot Local smoke after up-tes (health, lineage, preview, cohort, CWL + optional germline WES)"
	@echo "  make up-pilot  Start demo + ga4gh-infra with external auth (requires ../ga4gh-infra)"
	@echo "  make up-pilot-cloud  Local Ferrum + Fly ga4gh-infra/Keycloak (Fly must be running)"
	@echo "  make down      Stop stack; keep volumes"
	@echo "  make destroy   Stop stack; remove volumes and project images"
	@echo ""
	@echo "  make demo      Pull, build, start, wait for health"
	@echo "  make edge      Native single-binary Edge mode (no Docker)"
	@echo "  make laptop    Deprecated alias for make edge"
	@echo "  make logs      Tail compose logs"
	@echo "  make build     Build images only"

up: demo

# Demo + Docker TES: workflow engine images (pinned tags; amd64 on Apple Silicon).
TES_PLATFORM ?= linux/amd64
TES_IMAGES := \
	alpine:3.20 \
	quay.io/commonwl/cwltool:3.2.20260413085819 \
	nextflow/nextflow:24.10.3 \
	broadinstitute/cromwell:93-0232cbd \
	broadinstitute/gatk:4.4.0.0 \
	snakemake/snakemake:v7.32.4

up-tes:
	@mkdir -p deploy/.wes-runs
	@echo "Pulling TES workflow images (platform=$(TES_PLATFORM))..."
	@for img in $(TES_IMAGES); do \
		docker pull --platform $(TES_PLATFORM) $$img || echo "WARN: could not pull $$img"; \
	done
	$(COMPOSE_TES) pull
	$(COMPOSE_TES) up -d --build
	@echo "Waiting for gateway (max 90s)..."
	@for i in $$(seq 1 45); do \
		curl -sf http://localhost:$${GATEWAY_PORT:-8080}/health >/dev/null && echo "Gateway OK" && break; \
		[ $$i -eq 45 ] && echo "Gateway did not become healthy. Check: $(COMPOSE_TES) logs ferrum-gateway" && exit 1; \
		sleep 2; \
	done
	@echo ""
	@echo "Ferrum demo + TES (Docker) is up:"
	@echo "  Gateway: http://localhost:$${GATEWAY_PORT:-8080}"
	@echo "  UI:      http://localhost:$${UI_PORT:-8082}"
	@echo "  TES:     FERRUM_TES_BACKEND=docker — submit a run and watch docker ps during RUNNING"
	@command -v open >/dev/null 2>&1 && open "http://localhost:$${UI_PORT:-8082}/" || true

seed-pilot:
	@bash scripts/seed-pilot-demo.sh

smoke-pilot:
	@bash scripts/smoke-pilot-local.sh

# Pilot profile: Ferrum + ga4gh-infra AAI (mock-idp). Sibling ga4gh-infra checkout required.
up-pilot:
	@test -d "$(GA4GH_INFRA_SRC)" || (echo "GA4GH_INFRA_SRC not found: $(GA4GH_INFRA_SRC)" && exit 1)
	$(COMPOSE_PILOT) pull
	$(COMPOSE_PILOT) up -d --build
	@echo "Waiting for AAI broker (max 90s)..."
	@for i in $$(seq 1 45); do \
		curl -sf http://localhost:8180/service-info >/dev/null && echo "Broker OK" && break; \
		[ $$i -eq 45 ] && echo "Broker did not become healthy. Check: $(COMPOSE_PILOT) logs aai-broker" && exit 1; \
		sleep 2; \
	done
	@echo "Waiting for gateway (max 60s)..."
	@for i in $$(seq 1 30); do \
		curl -sf http://localhost:$${GATEWAY_PORT:-8080}/health >/dev/null && echo "Gateway OK" && break; \
		[ $$i -eq 30 ] && echo "Gateway did not become healthy. Check: $(COMPOSE_PILOT) logs ferrum-gateway" && exit 1; \
		sleep 2; \
	done
	@echo ""
	@echo "Ferrum pilot stack is up:"
	@echo "  Gateway:  http://localhost:$${GATEWAY_PORT:-8080}"
	@echo "  UI:       http://localhost:$${UI_PORT:-8082}/"
	@echo "  Broker:   http://localhost:8180/login/mock-idp (AAI sign-in for UI)"
	@command -v open >/dev/null 2>&1 && open "http://localhost:$${UI_PORT:-8082}/" || true

down-pilot:
	$(COMPOSE_PILOT) down

# Local Ferrum demo stack; auth via Fly pasteur-pilot ga4gh-infra + Keycloak (return_url localhost OK).
up-pilot-cloud:
	@test -d "$(GA4GH_INFRA_SRC)" || (echo "GA4GH_INFRA_SRC not found: $(GA4GH_INFRA_SRC)" && exit 1)
	@GA4GH_URL="$${PILOT_CLOUD_GA4GH_URL:-https://pasteur-pilot-ga4gh-infra.fly.dev}"; \
	  curl -sf "$$GA4GH_URL/service-info" >/dev/null \
	  || (echo "Fly broker not reachable at $$GA4GH_URL — run: pilot-deploy ./pilot.sh resume all --wait" && exit 1)
	$(COMPOSE_PILOT_CLOUD) pull
	$(COMPOSE_PILOT_CLOUD) up -d --build
	@echo "Waiting for gateway (max 90s)..."
	@for i in $$(seq 1 45); do \
		curl -sf http://localhost:$${GATEWAY_PORT:-8080}/health >/dev/null && echo "Gateway OK" && break; \
		[ $$i -eq 45 ] && echo "Gateway did not become healthy. Check: $(COMPOSE_PILOT_CLOUD) logs ferrum-gateway" && exit 1; \
		sleep 2; \
	done
	@GA4GH_URL="$${PILOT_CLOUD_GA4GH_URL:-https://pasteur-pilot-ga4gh-infra.fly.dev}"; \
	  echo ""; \
	  echo "Ferrum pilot-cloud is up (local data plane + Fly AAI):"; \
	  echo "  UI:       http://localhost:$${UI_PORT:-8082}/"; \
	  echo "  Gateway:  http://localhost:$${GATEWAY_PORT:-8080}"; \
	  echo "  Fly AAI:  $$GA4GH_URL/login/keycloak"; \
	  echo "  Sign in:  pasteur-demo-1 / PasteurDemo1!"; \
	  echo "  Smoke:    make test-pilot-cloud"
	@command -v open >/dev/null 2>&1 && open "http://localhost:$${UI_PORT:-8082}/" || true

down-pilot-cloud:
	$(COMPOSE_PILOT_CLOUD) down

down: stop

destroy: clean-all

# Optimized single-binary Laptop Mode (native CPU when possible)
edge:
	./scripts/build-edge-native.sh --install

laptop:
	@echo "make laptop is deprecated; use make edge" >&2
	$(MAKE) edge

# Pull images, build, start stack. Wait for gateway and UI to be reachable; fail with hint if not.
# Init seeds demo data (workspace, DRS, TRS, Keycloak). Use demo-user when auth is disabled.
demo:
	$(COMPOSE) pull
	$(COMPOSE) up -d --build
	@echo "Waiting for gateway (max 60s)..."
	@for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do \
		curl -sf http://localhost:$${GATEWAY_PORT:-8080}/health >/dev/null && echo "Gateway OK" && break; \
		[ $$i -eq 30 ] && echo "Gateway did not become healthy. Check: $(COMPOSE) logs ferrum-init ferrum-gateway" && exit 1; \
		sleep 2; \
	done
	@echo "Waiting for UI (max 30s)..."
	@for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do \
		curl -sf -o /dev/null http://localhost:$${UI_PORT:-8082}/ && echo "UI OK" && break; \
		[ $$i -eq 15 ] && echo "UI did not become reachable. Check: $(COMPOSE) logs ferrum-ui nginx" && exit 1; \
		sleep 2; \
	done
	@echo ""
	@echo "Ferrum demo is up:"
	@echo "  Gateway: http://localhost:$${GATEWAY_PORT:-8080}"
	@echo "  UI:      http://localhost:$${UI_PORT:-8082}"
	@command -v open >/dev/null 2>&1 && open "http://localhost:$${UI_PORT:-8082}" || true

# Stop all services
stop:
	$(COMPOSE) down

# Stop and remove volumes
clean: stop
	$(COMPOSE) down -v

# Full reset: stop, remove volumes, remove project images, prune build cache. Use before a completely fresh build.
clean-all: stop
	$(COMPOSE) down -v --remove-orphans
	-docker rmi -f ferrum-gateway:latest ferrum-ui:latest deploy-ferrum-init 2>/dev/null || true
	docker builder prune -f
	@echo "Clean complete. Run: make demo"

# Tail all logs
logs:
	$(COMPOSE) logs -f

# Build only (no start)
build:
	$(COMPOSE) build

# Force full rebuild of all images (no cache). Use after gateway/UI code changes.
rebuild:
	$(COMPOSE) build --no-cache
	@echo "Done. Start with: make demo"

# Force rebuild only gateway and UI (faster). Use when only Rust or frontend changed.
rebuild-gateway:
	$(COMPOSE) build --no-cache ferrum-gateway ferrum-ui
	@echo "Done. Restart with: $(COMPOSE) up -d"

# Pull only
pull:
	$(COMPOSE) pull

test-demo:
	chmod +x deploy/scripts/ci-docker-demo-e2e.sh
	./deploy/scripts/ci-docker-demo-e2e.sh

test-tes:
	chmod +x deploy/scripts/ci-docker-tes-e2e.sh deploy/scripts/ci-tes-pilot-e2e.sh
	./deploy/scripts/ci-docker-tes-e2e.sh

test-tes-full:
	./deploy/scripts/ci-tes-pilot-e2e.sh

test-pilot:
	chmod +x deploy/scripts/ci-pilot-aai-e2e.sh
	./deploy/scripts/ci-pilot-aai-e2e.sh

test-pilot-cloud:
	chmod +x deploy/scripts/ci-pilot-cloud-e2e.sh
	./deploy/scripts/ci-pilot-cloud-e2e.sh

test-federated:
	chmod +x deploy/scripts/ci-federated-e2e.sh
	./deploy/scripts/ci-federated-e2e.sh
