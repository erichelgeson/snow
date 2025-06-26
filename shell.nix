{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = with pkgs;
    [
      gcc

      cargo
      pkg-config
      SDL2
      SDL2.dev

      # For Wayland support
      wayland
      wayland-protocols
      libxkbcommon
      dbus
      mesa
      libglvnd
    ];

  shellHook = ''
    export LD_LIBRARY_PATH=${pkgs.wayland}/lib:${pkgs.libxkbcommon}/lib:${pkgs.mesa}/lib:${pkgs.mesa.drivers}:${pkgs.libglvnd}/lib:$LD_LIBRARY_PATH
  '';
}
