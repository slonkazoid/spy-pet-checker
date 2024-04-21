# spy-pet-checker

Check if any of the servers you are in is present in
[spy.pet](https://spy.pet/)'s database

## How to use

1. Download the tool from the [Releases](https://github.com/slonkazoid/spy-pet-checker/releases)
   tab, or [build from source](#build-from-source)
2. Start a terminal/command line in the directory with the executable
   and `index.json`
3. Run `./spy-pet-checker-x86_64-linux-gnu` (replace with the file you downloaded)

## How to obtain `index.json`

The official way is to get it from a discord data dump. On Discord, go to
User Settings -> Privacy & Safety and click "Request Data". The file will be in
the `servers` directory.

Another option, use the (upcoming) web version of this app.

## Build from source

You need `rustc` and `cargo` to build this project. The easiest way to get them
is to use [rustup](https://rustup.rs/).

```sh
git clone https://github.com/slonkazoid/spy-pet-checker
cd spy-pet-checker
cargo build --release
```

Your executable will be in `target/release/`.

