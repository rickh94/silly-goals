{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = [
    pkgs.openssl
    pkgs.pkg-config
    pkgs.clang
    pkgs.libiconv
    pkgs.sccache
    pkgs.bore-cli
    pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
    pkgs.mailhog
    pkgs.just
    pkgs.mkcert
    pkgs.caddy
  ];
}

