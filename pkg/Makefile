.SILENT:
.PHONY: default clean package build

default: build

clean:
	@echo "Cleaning up build artifacts..."
	@rm -f rustlens *.pkg.tar.zst *.tar.gz
	@rm -rf pkg src
	@echo "Cleanup complete."

package:
	@echo "Copying binary..."
	@cp ../target/release/rustlens .
	@echo "Updating version..."
	$(eval NEW_VERSION = $(shell cat ../Cargo.toml | grep '^version' | head -1 | awk -F'"' '{print $$2}'))
	@echo "Old version: $$(grep pkgver PKGBUILD | cut -d'=' -f2)"
	@sed -i "s/^pkgver=.*/pkgver=$(NEW_VERSION)/" PKGBUILD
	@echo "New version: $$(grep pkgver PKGBUILD | cut -d'=' -f2)"
	@echo "Updating package checksums..."
	@updpkgsums
	@echo "Building package..."
	@makepkg -f
	@echo "Package build complete."

build:
	@echo "Building binary..."
	@cd ..
	@cargo build --release
	@echo "Package build complete."
