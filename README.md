# WSLGit

Based on the great [WSLGit](https://github.com/andy-5/wslgit) project.

This project helps integrate Jetbrains IDEs with WSL.

## WARNING

The project may not work correctly with some commands (ex: blame, log) due to [WSL output being truncated](https://github.com/microsoft/WSL/issues/4082).

## Download

The latest binary release can be found on the
[releases page](https://github.com/Volune/wslgit-for-jetbrains/releases).

## How to use

- Download the latest version and extract it somewhere.
- Open the IDE's **settings**, go to **Version Control** and **Git**.
- Change the **Path to Git executable** to the extracted `wslgit-for-jetbrains.exe`.

## Not mounting in `/mnt`

The tool provides two additional commands that should help with most use cases:

The first will generate a mapping configuration file, the second will show the current configuration and the config file path.

```
wslgit-for-jetbrains.exe win-generate-mapping
```

```
wslgit-for-jetbrains.exe win-show-mapping
```

## Building from source

First, install Rust from https://www.rust-lang.org. Rust on Windows also
requires Visual Studio or the Visual C++ Build Tools for linking.

The final executable can then be build by running

```
cargo build --release
```

inside the root directory of this project. The resulting binary will
be located in `./target/release/`.

