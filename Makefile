PREFIX ?= /usr
DESTDIR ?=
GUI_BINARY ?= build/cmake-gui/wallfolio-gui

.PHONY: build test install
build:
	./scripts/cargo-safe build --release --workspace --locked
	./scripts/build-gui

test:
	./scripts/cargo-safe test --workspace --locked
	./scripts/cargo-safe build --workspace --locked
	python3 tests/smoke.py

install:
	install -Dm755 target/release/wallfolio $(DESTDIR)$(PREFIX)/bin/wallfolio
	install -Dm755 target/release/wallfoliod $(DESTDIR)$(PREFIX)/bin/wallfoliod
	install -Dm755 $(GUI_BINARY) $(DESTDIR)$(PREFIX)/bin/wallfolio-gui
	install -Dm644 packaging/desktop/io.wallfolio.Wallfolio.desktop $(DESTDIR)$(PREFIX)/share/applications/io.wallfolio.Wallfolio.desktop
	install -Dm644 assets/icons/io.wallfolio.Wallfolio.svg $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps/io.wallfolio.Wallfolio.svg
	install -d $(DESTDIR)$(PREFIX)/lib/systemd/user
	sed 's|/usr/bin/wallfoliod|$(PREFIX)/bin/wallfoliod|' packaging/systemd/wallfoliod.service > $(DESTDIR)$(PREFIX)/lib/systemd/user/wallfoliod.service
	install -Dm644 LICENSE $(DESTDIR)$(PREFIX)/share/licenses/wallfolio/LICENSE
	install -d $(DESTDIR)$(PREFIX)/share/bash-completion/completions $(DESTDIR)$(PREFIX)/share/zsh/site-functions $(DESTDIR)$(PREFIX)/share/fish/vendor_completions.d
	target/release/wallfolio completions bash > $(DESTDIR)$(PREFIX)/share/bash-completion/completions/wallfolio
	target/release/wallfolio completions zsh > $(DESTDIR)$(PREFIX)/share/zsh/site-functions/_wallfolio
	target/release/wallfolio completions fish > $(DESTDIR)$(PREFIX)/share/fish/vendor_completions.d/wallfolio.fish
