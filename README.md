# nuget2nix

Generates a Nix expression for `buildDotnetModule`, with support for non `nuget.org` repos.

## Usage

Similar to the `nuget-to-nix` command available in Nixpkgs, you'll need a directory of packages. This can be achieved with `dotnet restore` (see [here](https://github.com/NixOS/nixpkgs/blob/3ecddf791da4d893beb35fb09eb9da55b326f4fb/pkgs/build-support/build-dotnet-module/default.nix#L142) for an example). Additionally, you'll need a path to the `NuGet.config` file for your package.

Once you have these, the tool can be invoked like so:
```
$ nuget2nix --directory /path/to/packages --nuget-config /path/to/NuGet.config
```

On completion, the Nix expression to pass to `nugetDeps` is output to stdout.
