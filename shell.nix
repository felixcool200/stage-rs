{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    rustc
    cargo
    clippy
    rustfmt
    rust-analyzer

    # Native dependencies for git2/libgit2
    pkg-config
    openssl
    libgit2
    libssh2
    zlib

    # For clipboard support
    xclip
  ];

  OPENSSL_DIR = "${pkgs.openssl.dev}";
  OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
}
