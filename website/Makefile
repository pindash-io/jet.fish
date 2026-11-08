SHELL := /bin/sh

NPM ?= npm
WASM_PACK ?= wasm-pack
TERM_OUT := $(CURDIR)/public/assets/term

ASTRO_SITE ?=
ASTRO_BASE ?=
ASTRO_ARGS = $(if $(ASTRO_SITE),--site "$(ASTRO_SITE)") $(if $(ASTRO_BASE),--base "$(ASTRO_BASE)")

.PHONY: install ci-install term build dev preview clean verify ci

install:
	$(NPM) install

ci-install:
	$(NPM) ci

term:
	$(WASM_PACK) build term --target web --out-dir "$(TERM_OUT)"

build: term
	$(NPM) run build -- $(ASTRO_ARGS)

dev:
	$(NPM) run dev

preview:
	$(NPM) run preview

clean:
	rm -rf dist .astro term/pkg public/assets/term

verify:
	test -f public/assets/term/term.js
	test -f public/assets/term/term_bg.wasm
	test -f dist/assets/term/term.js
	test -f dist/assets/term/term_bg.wasm
	test -f dist/index.html

ci: ci-install build verify
