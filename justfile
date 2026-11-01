name := 'cosmic-ext-applet-timer'
appid := 'io.github.angelinuxx.CosmicExtAppletTimer'

cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
bin-src := cargo-target-dir / 'release' / name

user-base := env('HOME') / '.local'
user-bin-dst := user-base / 'bin' / name
user-desktop-dst := user-base / 'share' / 'applications' / appid + '.desktop'
user-metainfo-dst := user-base / 'share' / 'metainfo' / appid + '.metainfo.xml'
icon-name := appid + '-symbolic.svg'
icon-src := 'resources' / 'icons' / icon-name
user-icon-dst := user-base / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / icon-name

default: build-release

# Compiles with debug profile
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Symlinks the release binary into ~/.local for iterative development
dev-install: build-release
    mkdir -p {{user-base}}/bin
    ln -sf {{absolute_path(bin-src)}} {{user-bin-dst}}
    install -Dm0644 resources/app.desktop {{user-desktop-dst}}
    install -Dm0644 resources/app.metainfo.xml {{user-metainfo-dst}}
    install -Dm0644 {{icon-src}} {{user-icon-dst}}

# Rebuilds and restarts cosmic-panel to reload the applet
dev-reload: build-release
    pkill cosmic-panel || true

# Copies (not symlinks) into ~/.local
install-user: build-release
    install -Dm0755 {{bin-src}} {{user-bin-dst}}
    install -Dm0644 resources/app.desktop {{user-desktop-dst}}
    install -Dm0644 resources/app.metainfo.xml {{user-metainfo-dst}}
    install -Dm0644 {{icon-src}} {{user-icon-dst}}

# Removes user-local files
uninstall-user:
    rm -f {{user-bin-dst}} {{user-desktop-dst}} {{user-metainfo-dst}} {{user-icon-dst}}
