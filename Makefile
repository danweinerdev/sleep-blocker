# Build, test and package sleep-block.
#
#   make            # release build
#   make test       # run the test suite
#   make package    # build + test + produce a binary RPM
#   make install    # install into DESTDIR/PREFIX without RPM
#
# `package` produces a *binary* RPM: the compile happens here, with whatever
# toolchain is on PATH, and rpmbuild only stages the finished artefacts. That
# keeps a rustup toolchain usable, which a source RPM's BuildRequires would not.

NAME    := sleep-block
VERSION := $(shell sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)

PREFIX  ?= /usr
DESTDIR ?=

BINDIR   := $(DESTDIR)$(PREFIX)/bin
DATADIR  := $(DESTDIR)$(PREFIX)/share
ICONDIR  := $(DATADIR)/icons/hicolor
APPDIR   := $(DATADIR)/applications

BIN       := target/release/$(NAME)
ICON_SIZES := 16 22 24 32 48 64 128 256

# Keep RPM build output inside the project rather than polluting ~/rpmbuild.
# Override with `make package RPM_TOPDIR=/some/where` if you want the default.
RPM_TOPDIR ?= $(CURDIR)/tmp/rpmbuild
SPEC       := dist/rpm/$(NAME).spec
STAGE      := target/package/$(NAME)-$(VERSION)

.PHONY: all build test check clean install uninstall package srpm icons help

all: build

build:
	cargo build --release

test:
	cargo test --release

# Everything that must pass before shipping. Kept separate from `test` so the
# packaging path fails on a lint regression, not just a broken test.
check: test
	cargo clippy --release --all-targets -- -D warnings
	cargo fmt --check
	desktop-file-validate dist/$(NAME).desktop

# Regenerate the PNGs from the SVG sources. Not part of the default build: the
# PNGs are committed, and this needs ImageMagick.
#
# The 8-bit flags are load-bearing -- ImageMagick writes 16-bit PNGs by default
# and the tray decoder silently rejects them. See dist/icons/README.md.
icons:
	@command -v magick >/dev/null || { echo "error: ImageMagick not found"; exit 1; }
	@for state in active idle; do \
	    for size in $(ICON_SIZES); do \
	        magick -background none dist/icons/$(NAME)-$$state.svg \
	            -resize $${size}x$${size} \
	            -depth 8 -define png:color-type=6 -define png:bit-depth=8 \
	            PNG32:dist/icons/$(NAME)-$$state-$${size}.png; \
	    done; \
	done
	@echo "regenerated icons; run 'make test' to verify the format"

install: build
	install -Dpm0755 $(BIN) $(BINDIR)/$(NAME)
	install -Dpm0644 dist/$(NAME).desktop $(APPDIR)/$(NAME).desktop
	@for size in $(ICON_SIZES); do \
	    install -Dpm0644 dist/icons/$(NAME)-active-$${size}.png \
	        $(ICONDIR)/$${size}x$${size}/apps/$(NAME).png; \
	done
	install -Dpm0644 dist/icons/$(NAME)-active.svg \
	    $(ICONDIR)/scalable/apps/$(NAME).svg
	install -Dpm0644 LICENSE $(DATADIR)/licenses/$(NAME)/LICENSE
	@# Only refresh the caches for a real root install; a DESTDIR staging run
	@# is being packaged and the scriptlets belong to the package manager.
	if [ -z "$(DESTDIR)" ]; then \
	    update-desktop-database -q $(APPDIR) 2>/dev/null || true; \
	    touch --no-create $(ICONDIR) 2>/dev/null || true; \
	    gtk-update-icon-cache -qf $(ICONDIR) 2>/dev/null || true; \
	fi

uninstall:
	rm -f $(BINDIR)/$(NAME)
	rm -f $(APPDIR)/$(NAME).desktop
	@for size in $(ICON_SIZES); do \
	    rm -f $(ICONDIR)/$${size}x$${size}/apps/$(NAME).png; \
	done
	rm -f $(ICONDIR)/scalable/apps/$(NAME).svg
	rm -rf $(DATADIR)/licenses/$(NAME)

# Stage the built artefacts into the layout the spec's %install expects, then
# hand off to rpmbuild. The tarball carries binaries, not source.
package: check build
	@command -v rpmbuild >/dev/null || { echo "error: rpmbuild not found (dnf install rpm-build)"; exit 1; }
	rm -rf target/package
	mkdir -p $(STAGE)/icons
	install -pm0755 $(BIN)                  $(STAGE)/$(NAME)
	install -pm0644 dist/$(NAME).desktop    $(STAGE)/$(NAME).desktop
	install -pm0644 dist/icons/*.png        $(STAGE)/icons/
	install -pm0644 dist/icons/*.svg        $(STAGE)/icons/
	install -pm0644 LICENSE README.md       $(STAGE)/
	@# rpmbuild expects the whole tree, not just SOURCES, and will not create it.
	@# Listed individually rather than with brace expansion, which is a bashism
	@# and make runs recipes under /bin/sh.
	mkdir -p $(RPM_TOPDIR)/BUILD $(RPM_TOPDIR)/BUILDROOT $(RPM_TOPDIR)/RPMS \
	         $(RPM_TOPDIR)/SOURCES $(RPM_TOPDIR)/SPECS $(RPM_TOPDIR)/SRPMS
	tar -C target/package -czf $(RPM_TOPDIR)/SOURCES/$(NAME)-$(VERSION).tar.gz $(NAME)-$(VERSION)
	rpmbuild -bb --define '_topdir $(RPM_TOPDIR)' $(SPEC)
	@echo
	@echo "Built:"
	@find $(RPM_TOPDIR)/RPMS -name '$(NAME)-$(VERSION)*.rpm' | sed 's/^/  /'

clean:
	cargo clean
	rm -rf target/package $(RPM_TOPDIR)

help:
	@echo "make build     - release build"
	@echo "make test      - run tests"
	@echo "make check     - tests + clippy + fmt + desktop file validation"
	@echo "make package   - build, test, and produce a binary RPM"
	@echo "make install   - install to PREFIX (default /usr), honours DESTDIR"
	@echo "make icons     - regenerate PNG icons from SVG (needs ImageMagick)"
