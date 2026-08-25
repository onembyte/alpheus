# Maintainer: Franco Michetti <franco@example.com>
pkgname=alpheus
pkgver=0.8.0
pkgrel=1
pkgdesc="Honest, drillable storage manager, deep build cache cleaner, and disk monitor"
arch=('x86_64' 'aarch64')
url="https://github.com/onembyte/alpheus"
license=('MIT')
depends=('glibc' 'gcc-libs')
makedepends=('cargo' 'rust')
optdepends=(
    'pacman-contrib: paccache support for pacman cache cleaning'
    'flatpak: unused runtime cleanup'
    'docker: container and image cache pruning'
    'trash-cli: command line trash support'
)
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "$srcdir/$pkgname-$pkgver/src-tauri"
    export RUSTUP_TOOLCHAIN=stable
    cargo build --release --locked --bin alpheus
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    install -Dm755 "src-tauri/target/release/alpheus" "$pkgdir/usr/bin/alpheus"
    install -Dm644 "alpheus.desktop" "$pkgdir/usr/share/applications/alpheus.desktop"
    install -Dm644 "src-tauri/icons/128x128.png" "$pkgdir/usr/share/icons/hicolor/128x128/apps/alpheus.png"
    install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"

    # Shell completions
    "$pkgdir/usr/bin/alpheus" completion bash | install -Dm644 /dev/stdin "$pkgdir/usr/share/bash-completion/completions/alpheus"
    "$pkgdir/usr/bin/alpheus" completion zsh | install -Dm644 /dev/stdin "$pkgdir/usr/share/zsh/site-functions/_alpheus"
    "$pkgdir/usr/bin/alpheus" completion fish | install -Dm644 /dev/stdin "$pkgdir/usr/share/fish/vendor_completions.d/alpheus.fish"
}
