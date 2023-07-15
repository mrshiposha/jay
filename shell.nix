{ pkgs ? import <nixpkgs> {} }: pkgs.mkShell {
    nativeBuildInputs = with pkgs; [
        libinput
        cairo
        pango
        libGL
        systemd
        mesa
        libxkbcommon
    ];
}
