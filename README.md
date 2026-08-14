[![Build Status](https://travis-ci.org/neithanmo/csound-rs.svg?branch=master)](https://travis-ci.org/neithanmo/csound-rs) [![](https://img.shields.io/crates/v/csound.svg)](https://crates.io/crates/csound) 
# csound

[Csound](https://csound.com/) bindings for Rust.

Documentation can be found [*here*](https://neithanmo.github.io/csound-rs/csound/)


## Table of Contents
1. [Installation](#installation)
   1. [Linux/BSDs](#installation-linux)
   1. [macOS](#installation-macos)
   1. [Windows](#installation-windows)
1. [Getting Started](#getting-started)
1. [Running the tests](#testing)
1. [License](#license)
1. [Contribution](#contribution)

<a name="installation"/>

## Installation

The repo has git submodules, you need to initialize them:

```
$ git submodule init
$ git submodule update
```

To build the Csound bindings or anything depending on this crate, you need
**Csound 7.0**. Csound 6.x is *not* supported: this crate targets the Csound 7
host API, which removed and renamed a substantial part of the 6.x interface (see
the upstream [API migration guide](https://github.com/csound/csound/blob/develop/doc/API_Migration_Guide_Csound_6_to_7.md)).

Csound 7 has not been released yet, so it must be built from the `develop`
branch. The `csound-sys/csound` submodule pins the exact commit the bindings are
generated against — **build and link the same commit**, otherwise you risk
silent ABI mismatches.

By default( The only supported way), this crate will attempt to dynamically link to the system-wide libcsound64.

Bindgen needs `version.h` and `float-version.h`, which CMake generates at build
time and which are therefore absent from the source tree. Point bindgen at the
installed headers:

```
$ export BINDGEN_EXTRA_CLANG_ARGS="-I/path/to/csound/include"
```

<a name="installation-linux"/>

### Linux/BSDs

No distribution currently packages Csound 7, so you have to build it from
source.

```
# First, install all the csound's dependencies
$ apt-get install build-essential libportaudio2 portaudio19-dev cmake /
flex bison libsndfile1-dev libsndfile1
```
then, clone the csound's source code
```
# Clone Csound from its repository; Csound 7 lives on the develop branch
$ git clone -b develop https://github.com/csound/csound.git
```
Compile and install the library.

```
# Clone Csound from its repository
$ cd csound/
$ cmake . && make && sudo make install
$ sudo ldconfig
```
Csound will be installed in */usr/local/lib*, there is where the build.rs script will look at, for the csound's binaries.
so, It could be a good idea if you export this path in your bashrc or write a propper pkg-config file.

> [!NOTE]
> **Library configuration when compiled from source**
>
> To ensure the system can find the library in `/usr/local/lib`, follow these steps:
>
> 1. Create a configuration file with:
>    ```bash
>    sudo nano /etc/ld.so.conf.d/csound.conf
>    ```
>
> 2. Add this path to the file:
>    ```
>    /usr/local/lib
>    ```
>
> 3. Save the file and update the library cache:
>    ```bash
>    sudo ldconfig
>    ```


<a name="installation-macos"/>

### macOS

`CsoundLib64.framework` is expected in `/Library/Frameworks/`. If it's installed
in a different path specify `CSOUND_LIB_DIR` for that.

Csound's own CMake defaults to installing the framework into
`$HOME/Library/Frameworks`, which keeps a Csound 7 build clear of a system-wide
Csound 6 in `/Library/Frameworks`. A full build from a `develop` checkout:

```
$ brew install cmake ninja libsndfile bison
$ cd csound/
$ mkdir build && cd build
$ PATH="$(brew --prefix bison)/bin:$PATH" cmake .. -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=$HOME/csound7-install
$ ninja && ninja install
```

Homebrew's `bison` must precede the system one: macOS ships Bison 2.3 and Csound
requires 3.x.

Then build the bindings against that install:

```
$ export CSOUND_LIB_DIR=$HOME/Library/Frameworks
$ export BINDGEN_EXTRA_CLANG_ARGS="-I$CSOUND_LIB_DIR/CsoundLib64.framework/Versions/7.0/Headers"
$ cargo build
```

<a name="installation-windows"/>

### Windows

There is no Csound 7 installer yet, so build it from the `develop` branch with
CMake and install it locally.

1. Locate the directory holding `csound64.lib` in your Csound 7 install.
2. Open Command Prompt (make sure you Run as administrator so you're able to add a system environment variable).
3. Set the environment variable as follows:
```
$ setx CSOUND_LIB_DIR "C:\\path\\to\\csound7\\lib"
```
4. Restart Command Prompt to reload the environment variables then use the following command to check the it's been added correctly.
```
$ echo %CSOUND_LIB_DIR%
```
You should see the path to your Csound's lib installation. 


<a name="getting-started"/>

## Getting Started

The API reference can be found
[here](https://csound.com/docs/api/index.html)

For getting started withCsound-rs, you have to understand some basic concepts about Csound, before to try to use this
bindigs. Please check the Get Started page in the Csound's site
[Get Started](https://csound.com/get-started.html)
In addition there are csound api [examples](https://github.com/csound/csoundAPI_examples) inside of the rust directory.

<a name="license"/>

## Csound's examples for rust
The easy way to get familiar with csound is to explore the examples. To get the examples we just need to clone this repository.
```
# Clone Csound from its repository
$ git clone https://github.com/neithanmo/csound-rs.git
```
Now, go to the repository directory
```
# Clone Csound from its repository
$ cd csound-rs
```
For running the examples 1 to 10 just:
```
# Runs the example 5
$ cargo run --release --example example5
```
The  example 11 requires some dependencies, but you can run them through calling cargo on their own Cargo.toml file
```
# Runs the example 11
$ cd examples/example11
$ cargo --release build
$ cargo run
```

> [!NOTE]
> If bindgen fails with `'version.h' file not found`, it is because `version.h`
> and `float-version.h` are generated by CMake from their `.in` templates and so
> do not exist in a source checkout. Point `BINDGEN_EXTRA_CLANG_ARGS` at the
> headers of an installed Csound build rather than renaming files by hand.

<a name="testing"/>

## Running the tests

```
$ export CSOUND_LIB_DIR=...        # see the platform sections above
$ export BINDGEN_EXTRA_CLANG_ARGS=-I...
$ cargo test --workspace
```

The suite includes **differential tests** (`tests/differential.rs`) that render
the same `.csd` twice — once with the `csound` command-line frontend, once
through the bindings — and compare the resulting samples bit for bit. They exist
to catch the failure mode this crate is most exposed to: a binding that compiles
and runs but is subtly wrong, such as a changed signature or a buffer read at
the wrong rate. Those mistakes type-check cleanly and then corrupt audio.

They need a `csound` binary built from the *same* source as the linked library:

```
$ export CSOUND_BIN=$HOME/csound7-install/bin/csound
```

If no suitable binary is found the differential tests skip. If one is found but
reports a different version than the linked library, they fail rather than
compare across versions, since that would not prove anything.

## License

csound-rs is licensed under either
* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

 at your option.

 Csound itself is licensed under the Lesser General Public License version
 2.1 or (at your option) any later version:
 https://www.gnu.org/licenses/lgpl-2.1.html

 <a name="contribution"/>

 ## Contribution

 Any kinds of contributions are welcome as a pull request.

 Unless you explicitly state otherwise, any contribution intentionally submitted
 for inclusion in csound-rs by you, as defined in the Apache-2.0 license, shall be
 dual licensed as above, without any additional terms or conditions.
