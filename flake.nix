{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { self, nixpkgs }:
    let
      supportedSystems = [
        "x86_64-linux"
        "x86_64-darwin"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgs = forAllSystems (system: nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (
        system:
        let
          package = pkgs.${system}.rustPlatform.buildRustPackage {
            pname = "ts3status-rs";
            version = "0.1.0";
            src = ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
          };
        in
        {
          default = package;
          ts3status-rs = package;
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.ts3status-rs}/bin/ts3status-rs";
        };
      });

      devShells = forAllSystems (system: {
        default = pkgs.${system}.mkShell {
          buildInputs = with pkgs.${system}; [
            cargo
            rustc
            rust-analyzer
            rustfmt
          ];
          DATABASE_PATH = "dev.sqlite3";
          shellHook = ''
            export TS3STATUS_CONFIG_PATH="$(pwd)/dev_config.toml"
          '';
        };
      });
    };
}
