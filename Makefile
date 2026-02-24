# Ferrum – local demo and dev targets
# Run from repo root.

COMPOSE_FILE := deploy/docker-compose.yml
COMPOSE := docker compose -f $(COMPOSE_FILE)

.PHONY: demo stop clean logs pull build

# Pull images, build, start stack, open browser
demo:
	$(COMPOSE) pull
	$(COMPOSE) up -d --build
	@echo "Waiting for gateway..."
	@for i in 1 2 3 4 5 6 7 8 9 10; do \
		curl -sf http://localhost:$${GATEWAY_PORT:-8080}/health >/dev/null && break; \
		sleep 2; \
	done
	@echo "Ferrum demo up: gateway http://localhost:$${GATEWAY_PORT:-8080}  UI http://localhost:$${UI_PORT:-8082}"
	@command -v open >/dev/null 2>&1 && open "http://localhost:$${UI_PORT:-8082}" || true

# Stop all services
stop:
	$(COMPOSE) down

# Stop and remove volumes
clean: stop
	$(COMPOSE) down -v

# Tail all logs
logs:
	$(COMPOSE) logs -f

# Build only (no start)
build:
	$(COMPOSE) build

# Pull only
pull:
	$(COMPOSE) pull
