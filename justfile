build:
    cargo build --release
    mkdir -p ~/.local/bin
    cp target/release/pdiff ~/.local/bin/
    @echo "Installed pdiff to ~/.local/bin/"

install-pi:
    ~/.local/bin/pdiff install pi
    @echo "Pi extension installed"

install: build install-pi
