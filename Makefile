# Ferrum – local demo and dev targets
# Run from repo root.

COMPOSE_FILE := deploy/docker-compose.yml
COMPOSE := docker compose -f $(COMPOSE_FILE)

.PHONY: demo stop clean clean-all logs pull build rebuild rebuild-gateway

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
