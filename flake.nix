{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rtabmap-src = {
      url = "github:Pylgos/rtabmap/cpp20";
      flake = false;
    };
  };

  outputs =
    {
      self,
      flake-utils,
      nixpkgs,
      rtabmap-src,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        lib = nixpkgs.lib;
        pkgs = nixpkgs.legacyPackages.${system};
        selfPackages = self.packages.${system};
      in
      {
        devShells.default =
          (pkgs.mkShell.override (_: {
            stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.clangStdenv;
          }))
            rec {
              buildInputs = [
                pkgs.cmake
                pkgs.godot_4
                pkgs.libGL
                pkgs.libxkbcommon
                pkgs.llvmPackages.clang-unwrapped.lib
                pkgs.meson
                pkgs.pkg-config
                pkgs.protobuf
                pkgs.wayland
                pkgs.wayland-protocols
                pkgs.xorg.libX11
                pkgs.xorg.libXcursor
                pkgs.xorg.libXi
                pkgs.xorg.libXrandr
                selfPackages.rtabmap
              ];
              LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
              LIBCLANG_PATH = "${pkgs.llvmPackages.clang-unwrapped.lib}/lib";
            };
        packages = {
          rtabmap = pkgs.rtabmap.overrideAttrs (oldAttrs: {
            src = rtabmap-src;
            propagatedBuildInputs = oldAttrs.buildInputs ++ [ selfPackages.librealsense ];
          });
          librealsense = (
            pkgs.librealsense-gui.overrideAttrs (oldAttrs: {
              version = "2.55.1";
              src = pkgs.fetchFromGitHub {
                owner = "IntelRealSense";
                repo = "librealsense";
                rev = "v2.55.1";
                hash = "sha256-MNHvfWk58WRtu6Xysfvn+lx8J1+HlNw5AmmgaTAzuok=";
              };
              patches = lib.filter (
                p: !(lib.hasInfix "fix-gcc13-missing-cstdint.patch" (builtins.baseNameOf p))
              ) oldAttrs.patches;
              postPatch =
                (oldAttrs.postPatch or "")
                + ''
                  substituteInPlace CMake/json-download.cmake.in --replace 'GIT_REPOSITORY "https://github.com/nlohmann/json.git"' 'URL ${
                    pkgs.fetchFromGitHub {
                      owner = "nlohmann";
                      repo = "json";
                      rev = "v3.11.3";
                      hash = "sha256-7F0Jon+1oWL7uqet5i1IgHX0fUw/+z0QwEcA3zs5xHg=";
                    }
                  }'
                '';
            })
          );
        };
      }
    );
}
