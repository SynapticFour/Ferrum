# Ferrum – local demo and dev targets
# Run from repo root.

COMPOSE_FILE := deploy/docker-compose.yml
COMPOSE := docker compose -f $(COMPOSE_FILE)
GA4GH_INFRA_SRC ?= $(abspath ../ga4gh-infra)
export GA4GH_INFRA_SRC
COMPOSE_PILOT := docker compose -f deploy/docker-compose.yml -f deploy/docker-compose.ga4gh-infra.yml -f deploy/docker-compose.pilot.yml

.PHONY: help up down destroy demo stop clean clean-all logs pull build rebuild rebuild-gateway laptop up-pilot down-pilot

# Synaptic Four unified local lifecycle: up → down → destroy
help:
	@echo "Ferrum — local lifecycle (Synaptic Four GA4GH stack)"
	@echo ""
	@echo "  make up        Start demo stack (alias: make demo)"
	@echo "  make up-pilot  Start demo + ga4gh-infra with external auth (requires ../ga4gh-infra)"
	@echo "  make down      Stop stack; keep volumes"
	@echo "  make destroy   Stop stack; remove volumes and project images"
	@echo ""
	@echo "  make demo      Pull, build, start, wait for health"
	@echo "  make laptop    Native single-binary laptop mode (no Docker)"
	@echo "  make logs      Tail compose logs"
	@echo "  make build     Build images only"

up: demo

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

down: stop

destroy: clean-all

# Optimized single-binary Laptop Mode (native CPU when possible)
laptop:
	./scripts/build-laptop-native.sh --install

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
