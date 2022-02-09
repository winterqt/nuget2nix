{
  description = "Generates a Nix expression for `buildDotnetModule`";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
      inherit (pkgs) lib stdenv;
    in
    {
      packages.nuget2nix = pkgs.rustPlatform.buildRustPackage {
        pname = "nuget2nix";
        version = (lib.importTOML ./Cargo.toml).package.version;

        src = self;
        cargoLock.lockFile = ./Cargo.lock;

        buildInputs = lib.optional stdenv.isDarwin pkgs.darwin.apple_sdk.frameworks.Security;
      };
      defaultPackage = self.packages.${system}.nuget2nix;

      apps.nuget2nix = {
        type = "app";
        program = "${self.packages.${system}.nuget2nix}/bin/nuget2nix";
      };
      defaultApp = self.apps.${system}.nuget2nix;

      devShell = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [ rustc cargo clippy ] ++ lib.optionals stdenv.isDarwin [ libiconv darwin.apple_sdk.frameworks.Security ];
      };
    }
  );
}
