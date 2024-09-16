# Makefile para Windows con Docker

# Configura la ruta al proyecto en formato de ruta de Docker
PROJECT_NAME = crypted-messages

PROJECT_DIR = /x/Programacion/otros/$(PROJECT_NAME)

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

# Actualiza la imagen de Docker
update-image:
	@docker pull joseluisq/rust-linux-darwin-builder
.PHONY: update-image

# Actualiza rustc en el contenedor
update-rustc:
	@docker run --rm -it \
		-v $(PROJECT_DIR):/drone/src \
		-w /drone/src \
		joseluisq/rust-linux-darwin-builder:latest \
		rustup update
.PHONY: update-rustc

# Compilar todos los objetivos
compile: update-image update-rustc
	@docker run --rm -it \
		-v $(PROJECT_DIR):/drone/src \
		-w /drone/src \
		joseluisq/rust-linux-darwin-builder:latest \
		make cross-compile

# Regla para compilar y renombrar el binario
cross-compile:
	@echo -e "$(CYAN)Compiling targets...$(RESET)"
	@successful_targets="" && \
	for target in $(TARGETS); do \
		echo -e "$(YELLOW)Building for $$target...$(RESET)" && \
		if cargo build --release --target $$target; then \
			successful_targets="$$successful_targets $$target"; \
			binary_path=target/$$target/release/$(PROJECT_NAME); \
			if [ -f $$binary_path ]; then \
				mv $$binary_path target/$$target/release/$(PROJECT_NAME)-$$target; \
				echo -e "$(GREEN)Renamed to $(PROJECT_NAME)-$$target$(RESET)"; \
			else \
				echo -e "$(RED)Binary not found for $$target$(RESET)"; \
			fi; \
		else \
			echo -e "$(RED)Build failed for $$target$(RESET)"; \
		fi; \
	done; \
	echo; \
	echo -e "$(GREEN)Build completed!$(RESET)"; \
	echo -e "$(CYAN)Successful targets:$(RESET)"; \
	for target in $$successful_targets; do \
		echo -e "$(BOLD)$$target$(RESET)"; \
	done
.PHONY: cross-compile
