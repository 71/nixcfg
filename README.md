nixcfg
======

Command line utility to query and modify .nix files.

## Usage

```
nixcfg 0.1.0
Grégoire Geis <git@gregoirege.is>
Command line utility to query and modify .nix files.

USAGE:
    nixcfg [FLAGS] [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help        Prints help information
    -i, --in-place    Modify in place instead of printing result to stdout.
    -V, --version     Prints version information

OPTIONS:
    -f, --file <input>    Input .nix file to query or modify. [default: /etc/nixos/configuration.nix]

SUBCOMMANDS:
    get     Get the value at the given path.
    set     Set the value at the given path.
```

## Examples

Let's consider the following file:

```nix
# file.nix
{ pkgs, config, ... }:

{
  environment.systemPackages = with pkgs; [ ];

  networking.firewall.enable = true;
  networking.firewall.allowedTCPPorts = [ 80 8080 8000 24800 ];

  nixpkgs.config = { allowBroken = false; allowUnfree = true; };
}
```

### Querying values
- `nixpkg -f file.nix get environment.systemPackages` yields `with pkgs; [ ]`.
- `nixpkg -f file.nix get nixpkgs.config` yields `{ allowBroken = false; allowUnfree = true; }`.
- `nixpkg -f file.nix get nixpkgs.config.allowBroken` yields `false`.

**Please note that `nixcfg` does not recognize values belonging to a same object.**
- `nixpkg -f file.nix get networking.firewall.enable` yields `true`.
- `nixpkg -f file.nix get networking.firewall.allowedTCPPorts` yields `[ 80 8080 8000 24800 ]`.
- `nixpkg -f file.nix get networking.firewall` fails to find a matching value.

### Updating values
`nixpkg -f file.nix set networking.firewall.enable false`

yields

```nix
# file.nix
{ pkgs, config, ... }:

{
  environment.systemPackages = with pkgs; [ ];

  networking.firewall.enable = false;
  networking.firewall.allowedTCPPorts = [ 80 8080 8000 24800 ];

  nixpkgs.config = { allowBroken = false; allowUnfree = true; };
}
```

## To do
- [ ] Add ability to insert the value, if the key did not previously exist.

## Disclaimer

This project is very new, and has only been tested in limited test suites.  
Use at your own risks.
