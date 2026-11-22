# app-shell's dev environment: devenv-fleet's Rust module (the 1.99.1 pin,
# mold, kache as the wrapper), and what app-shell-native (Blitz, winit,
# wgpu) needs to build and to open a window on Linux. dx 0.8 for a web
# build is the fleet's (README). On macOS the system frameworks are all
# the native side needs.
{ pkgs, lib, ... }:
let
  linux = lib.optionals pkgs.stdenv.isLinux (with pkgs; [
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
    # app-shell-native's `tray` feature: wry's web views (WebKitGTK).
    gtk3
    webkitgtk_4_1
    libsoup_3
    glib
    cairo
    pango
    gdk-pixbuf
    atk
    harfbuzz
  ]);
in
{
  packages = [ pkgs.pkg-config ] ++ linux ++ map lib.getDev linux;
  env.LD_LIBRARY_PATH = lib.optionalString pkgs.stdenv.isLinux (lib.makeLibraryPath linux);
}
