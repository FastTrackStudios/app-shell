# app-shell's dev environment: devenv-fleet's Rust module (the 1.99.1 pin,
# mold, kache as the wrapper), and what app-shell-native (Blitz, winit,
# wgpu) needs to build and to open a window on Linux. dx 0.8 for a web
# build is the fleet's (README).
{ pkgs, lib, ... }:
let
  runtime = with pkgs; [
    fontconfig
    freetype
    libxkbcommon
    wayland
    vulkan-loader
    libGL
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
    xorg.libXrandr
  ];
in
{
  packages = [ pkgs.pkg-config pkgs.fontconfig.dev pkgs.freetype.dev ] ++ runtime;
  env.LD_LIBRARY_PATH = lib.makeLibraryPath runtime;
}
