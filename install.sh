#!/usr/bin/env bash
# Install script for cabalist
# Usage: curl -fsSL https://raw.githubusercontent.com/joshburgess/cabalist/main/install.sh | bash

set -euo pipefail

REPO="joshburgess/cabalist"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

info() {
    printf "\033[1;34m==>\033[0m %s\n" "$1"
}

error() {
    printf "\033[1;31merror:\033[0m %s\n" "$1" >&2
    exit 1
}

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)
            case "$arch" in
                x86_64)  echo "x86_64-unknown-linux-musl" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                arm64)   echo "aarch64-unknown-linux-gnu" ;;
                *) error "Unsupported architecture: $arch" ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                x86_64) echo "x86_64-apple-darwin" ;;
                arm64)  echo "aarch64-apple-darwin" ;;
                *) error "Unsupported architecture: $arch" ;;
            esac
            ;;
        *) error "Unsupported OS: $os" ;;
    esac
}

get_latest_version() {
    local url="https://api.github.com/repos/${REPO}/releases/latest"
    if command -v curl &>/dev/null; then
        curl -fsSL "$url" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p'
    elif command -v wget &>/dev/null; then
        wget -qO- "$url" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p'
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

download() {
    local url="$1" dest="$2"
    if command -v curl &>/dev/null; then
        curl -fsSL -o "$dest" "$url"
    elif command -v wget &>/dev/null; then
        wget -qO "$dest" "$url"
    fi
}

main() {
    local platform version archive_url tmpdir

    platform="$(detect_platform)"
    info "Detected platform: $platform"

    info "Fetching latest release..."
    version="$(get_latest_version)"
    if [ -z "$version" ]; then
        error "Could not determine latest version. Check https://github.com/${REPO}/releases"
    fi
    info "Latest version: $version"

    archive_url="https://github.com/${REPO}/releases/download/${version}/cabalist-${version}-${platform}.tar.gz"
    info "Downloading ${archive_url}..."

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    download "$archive_url" "$tmpdir/cabalist.tar.gz"

    info "Extracting..."
    tar xzf "$tmpdir/cabalist.tar.gz" -C "$tmpdir"

    info "Installing to ${INSTALL_DIR}..."
    mkdir -p "$INSTALL_DIR"

    for bin in cabalist cabalist-cli; do
        if [ -f "$tmpdir/$bin" ]; then
            install -m 755 "$tmpdir/$bin" "$INSTALL_DIR/$bin"
            info "Installed $bin"
        fi
    done

    # Check if INSTALL_DIR is in PATH
    case ":$PATH:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            printf "\n"
            info "Add %s to your PATH to use cabalist:" "$INSTALL_DIR"
            printf "    export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
            ;;
    esac

    printf "\n"
    info "Installation complete! Run 'cabalist --help' to get started."
}

main "$@"
