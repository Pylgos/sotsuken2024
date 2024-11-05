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
        pkgs = import nixpkgs {
          config.android_sdk.accept_license = true;
          config.allowUnfree = true;
          inherit system;
        };
        androidPkgs = pkgs.androidenv.composeAndroidPackages {
          includeEmulator = false;
          platformVersions = [
            "34"
          ];
          buildToolsVersions = [ "34.0.0" ];
          includeSources = false;
          includeSystemImages = false;
          abiVersions = [
            "arm64-v8a"
          ];
          includeNDK = true;
          ndkVersions = [ "23.2.8568313" ];
          useGoogleAPIs = false;
          useGoogleTVAddOns = false;
        };
        selfPackages = self.packages.${system};
        mkShell = pkgs.mkShell.override (_: {
          stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.clangStdenv;
        });
      in
      {
        devShells.default = mkShell rec {
          buildInputs = [
            pkgs.cmake
            pkgs.godot_4
            pkgs.jdk17
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
          ANDROID_HOME = "${androidPkgs.androidsdk}/libexec/android-sdk";
          JAVA_HOME = "${pkgs.jdk17}";
          CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = "${ANDROID_HOME}/ndk-bundle/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang";
          CARGO_TARGET_AARCH64_LINUX_ANDROID_AR = "${ANDROID_HOME}/ndk-bundle/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar";
          GODOT_ANDROID_KEYSTORE_DEBUG_USER = "android";
          GODOT_ANDROID_KEYSTORE_DEBUG_PASSWORD = "android";
          GODOT_ANDROID_KEYSTORE_DEBUG_PATH = "res://debug.keystore";
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
