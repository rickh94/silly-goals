{ pkgs, config, ... }:

{
  # https://devenv.sh/basics/
  env.GREET = "devenv";

  # https://devenv.sh/packages/
  packages = [
    pkgs.openssl
    pkgs.pkg-config
    pkgs.libiconv
    pkgs.sccache
    pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
    pkgs.darwin.apple_sdk.frameworks.CoreFoundation
    pkgs.darwin.apple_sdk.frameworks.Security
    pkgs.mailhog
    pkgs.just
    pkgs.gcc
  ];

  # https://devenv.sh/languages/
  # languages.nix.enable = true;
  languages = {
    rust.enable = true;
  };

  certificates = [
    "silly.localhost"
  ];

  services.caddy = {
    enable = true;
    virtualHosts."silly.localhost" = {
      extraConfig = ''
        reverse_proxy :8000
      '';
    };
  };

  services.redis = {
    enable = true;
    port = 6388;
  };

}
