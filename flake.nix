# flake.nix
{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        naersk' = pkgs.callPackage naersk { };

      in
      rec {
        # For `nix develop` (optional, can be skipped):
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [ rustc cargo ];
        };

        packages = rec {
          default = naersk'.buildPackage {
            src = ./.;

            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = with pkgs; [ openssl ];
          };

          docker = pkgs.dockerTools.buildImage {
            name = "calendar-join";
            tag = "latest";

            config = {
              Env = [ "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" ];
              Cmd = [ "${default}/bin/calendar-join" ];
              WorkingDir = "/data";
              ExposedPorts = {
                "8080/tcp" = { };
              };
            };
          };
        };
      }
    );
}
