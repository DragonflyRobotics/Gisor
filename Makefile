# Usage:
#   make run RUNFILE=<runfile> PTX_FILE=<ptx_file>
#   make run_release RUNFILE=<runfile> PTX_FILE=<ptx_file>

RUNFILE= default1
PTX_FILE= default2

all:
	@echo "Building..."
	$(MAKE) build
	@echo "Done."
run:
	@echo "Running..."
	@echo "Runfile: $(RUNFILE), PTX file: $(PTX_FILE)"
	cargo build
	cargo run -p xtask -- launch -- $(RUNFILE) $(PTX_FILE)
run_release:
	cargo build --release
	cargo run -p xtask -- launch --release -- $(RUNFILE) $(PTX_FILE)
build:
	cargo build
build_release:
	cargo build --release
test:
	@echo "Testing..."
	cargo test
clean:
	cargo clean