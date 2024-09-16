# Makefile para Windows con Docker

# Configura la ruta al proyecto en formato de ruta de Docker
PROJECT_DIR = /x/Programacion/otros/crypted-messages

# Lista de objetivos
TARGETS = x86_64-unknown-linux-musl x86_64-unknown-linux-gnu x86_64-apple-darwin \
          aarch64-unknown-linux-musl aarch64-unknown-linux-gnu aarch64-apple-darwin

# Colores y estilos
RESET = \033[0m
BOLD = \033[1m
UNDERLINE = \033[4m
GREEN = \033[32m
RED = \033[31m
YELLOW = \033[33m
CYAN = \033[36m

# Puedes agregar una regla para asegurarte de que la imagen esté actualizada
update-image:
	@docker pull joseluisq/rust-linux-darwin-builder
.PHONY: update-image

# Compilar todos los objetivos y mostrar los exitosos
compile: update-image
	@docker run --rm -it \
		-v $(PROJECT_DIR):/drone/src \
		-w /drone/src \
		joseluisq/rust-linux-darwin-builder:latest \
		make cross-compile

# Regla para compilar con diferentes objetivos
cross-compile:
	@echo -e "$(CYAN)Compiling targets...$(RESET)"
	@successful_targets="" && \
	for target in $(TARGETS); do \
		echo -e "$(YELLOW)Building for $$target...$(RESET)" && \
		if cargo build --release --target $$target; then \
			successful_targets="$$successful_targets $$target"; \
		else \
			echo -e "$(RED)Build failed for $$target$(RESET)"; \
		fi; \
	done && \
	echo && \
	echo -e "$(GREEN)Build completed!$(RESET)" && \
	echo -e "$(CYAN)Successful targets:$(RESET)" && \
	for target in $$successful_targets; do \
		echo -e "$(BOLD)$$target$(RESET)"; \
	done
.PHONY: cross-compile
