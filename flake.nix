{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/24.05";
    rtabmap-src = {
      url = "github:Pylgos/rtabmap/cpp20";
      flake = false;
    };
  };

  outputs = {self, flake-utils, nixpkgs, rtabmap-src}:
    flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = nixpkgs.legacyPackages.${system};
      selfPackages = self.packages.${system};
    in {
      devShells.default = (pkgs.mkShell.override (_: { stdenv = pkgs.gcc13Stdenv; })) {
        nativeBuildInputs = [
          pkgs.meson
          pkgs.pkg-config
          pkgs.cmake
          pkgs.llvmPackages.clang-unwrapped.lib
        ];
        buildInputs = [
          selfPackages.rtabmap
        ];
        LIBCLANG_PATH = "${pkgs.llvmPackages.clang-unwrapped.lib}/lib";
      };
      packages = {
        rtabmap = pkgs.rtabmap.overrideAttrs (oldAttrs: {
            src = rtabmap-src;
            propagatedBuildInputs = oldAttrs.buildInputs ++ [
              pkgs.librealsense
            ];
          });
      };
    }
  );
}