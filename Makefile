.PHONY: golden golden-latest test-protocol test-all

# make golden TRACE_ID=<id> — из .papa-yu/traces/<id>.json
# make golden — из последней трассы (golden-latest)
golden:
	@if [ -n "$$TRACE_ID" ]; then \
		cd src-tauri && cargo run --bin trace_to_golden -- "$$TRACE_ID"; \
	else \
		$(MAKE) golden-latest; \
	fi

golden-latest:
	@LATEST=$$(ls -t .papa-yu/traces/*.json 2>/dev/null | head -1); \
	if [ -z "$$LATEST" ]; then \
		echo "No traces in .papa-yu/traces/. Run with PAPAYU_TRACE=1, propose fixes, then make golden."; \
		exit 1; \
	fi; \
	cd src-tauri && cargo run --bin trace_to_golden -- "../$$LATEST"

test-protocol:
	cd src-tauri && cargo test golden_traces_v1_validate

test-all:
	cd src-tauri && cargo test
