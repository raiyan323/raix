pkgname=raix
pkgver=0.1.0
pkgrel=1
pkgdesc="Fast and minimal Wayland application launcher"
arch=('x86_64')
url="https://github.com/raiyan323/raix"
license=('MIT')

depends=('wayland')
makedepends=('cargo' 'git')

source=("git+https://github.com/raiyan323/raix.git")
sha256sums=('SKIP')

prepare() {
    cd "$srcdir/raix"
    export CARGO_HOME="$srcdir/cargo"
    cargo fetch
}

build() {
    cd "$srcdir/raix"
    export CARGO_HOME="$srcdir/cargo"
    cargo build --release
}

package() {
    cd "$srcdir/raix"

    install -Dm755 \
        "target/release/raix" \
        "$pkgdir/usr/bin/raix"
}