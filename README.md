[![Build Status](https://travis-ci.org/neithanmo/csound-rs.svg?branch=master)](https://travis-ci.org/neithanmo/csound-rs) [![](https://img.shields.io/crates/v/csound.svg)](https://crates.io/crates/csound) 
# csound

[Csound](https://csound.com/) bindings for Rust.

Documentation can be found [*here*](https://neithanmo.github.io/csound-rs/csound/)


## Table of Contents
1. [Installation](#installation)
   1. [Linux](#installation-linux)
   1. [macOS](#installation-macos)
   1. [Windows](#installation-windows)
1. [Getting Started](#getting-started)
1. [Running the tests](#testing)
1. [License](#license)
1. [Contribution](#contribution)

<a name="installation"/>

## Installation

To build the Csound bindings or anything depending on this crate, you need a
**Csound 7.0 or newer development installation**. Csound 6.x is not supported:
this crate targets the Csound 7 host API, which removed and renamed a
substantial part of the 6.x interface (see the upstream
[API migration guide](https://github.com/csound/csound/blob/develop/doc/API_Migration_Guide_Csound_6_to_7.md)).

On Linux, `csound-sys` generates bindings from the installed Csound headers and
dynamically links the matching system `libcsound64`. A normal user therefore
does not need to initialize the Csound Git submodule. The submodule pins the
source revision built by this repository's CI and is needed by maintainers when
building that pinned Csound revision.

Linux discovery tries the `csound` pkg-config package first and requires version
7.0 or newer. If pkg-config is unavailable, it checks the conventional
`/usr/local` and `/usr` include and library paths. For a custom installation,
set `CSOUND_INCLUDE_DIR` to the directory containing `csound.h` and
`CSOUND_LIB_DIR` to the directory containing `libcsound64.so`.

<a name="installation-linux"/>

### Linux

Install Csound 7 and its development files through your distribution when they
are available. A source installation can be built with CMake:

```
$ git clone https://github.com/csound/csound.git
$ cd csound/
$ cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
$ cmake --build build --parallel
$ sudo cmake --install build
$ sudo ldconfig
```

A complete Csound 7 installation includes `csound.pc`, the public headers
(including the CMake-generated `version.h` and `float-version.h`), and
`libcsound64`. Source installations normally use `/usr/local`; the build script
checks that prefix if pkg-config is unavailable.

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

> [!NOTE]
> Csound 7's framework records an `@rpath`-relative install name
> (`@rpath/CsoundLib64.framework/Versions/7.0/CsoundLib64`). Executables linking
> it need a matching `LC_RPATH` or dyld fails at load time with
> `no LC_RPATH's found`. This crate's build script emits that rpath for its own
> tests and examples, and republishes the framework directory to dependents as
> `DEP_CSOUND64_FRAMEWORK_DIR`. **If you build an executable against this crate,
> add a `build.rs` that re-emits it:**
>
> ```rust
> fn main() {
>     if cfg!(target_os = "macos") {
>         if let Ok(dir) = std::env::var("DEP_CSOUND64_FRAMEWORK_DIR") {
>             println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir);
>         }
>     }
> }
> ```

<a name="installation-windows"/>

### Windows

The build script first looks for a Csound 7 development installation under:

```text
C:\Program Files\Csound
C:\Program Files\Csound7_x64
C:\Program Files\Csound6_x64
```

The legacy-named `Csound6_x64` location is checked for compatibility with
existing installation layouts, but its `version.h` must still report Csound 7
or newer. Headers may be in `include` or `include\csound`; `csound64.lib` may be
in `lib` or `bin`.

For a custom installation, set both paths and restart the shell:

```console
setx CSOUND_INCLUDE_DIR "C:\path\to\csound7\include"
setx CSOUND_LIB_DIR "C:\path\to\csound7\lib"
```

The directory containing `csound64.dll` must also be present in `PATH` when
running tests, examples, or applications.


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
> On Linux, bindgen uses the installed Csound headers discovered through
> pkg-config or the fallback paths described above. `version.h` and
> `float-version.h` are generated and installed by Csound's CMake build; they do
> not exist in an unconfigured Csound source checkout. The existing macOS build
> still uses `BINDGEN_EXTRA_CLANG_ARGS` when an additional header search path is
> required.

<a name="testing"/>

## Running the tests

On Linux with a standard Csound 7 development installation:

```
$ cargo test --workspace
```

For a custom Linux installation, set `CSOUND_LIB_DIR` and
`CSOUND_INCLUDE_DIR` as described above. See the platform sections for macOS and
Windows setup.

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

### Miri and AddressSanitizer

```
$ just miri     # undefined behaviour in the callback trampolines' pointer handling
$ just asan     # the same trampolines running for real, under AddressSanitizer
```

The two are complementary because Miri cannot cross the FFI boundary: it
refuses to call foreign functions, so it cannot execute a trampoline (each one
begins with `csoundGetHostData`) nor any test that constructs a `Csound`. The
unsafe core those trampolines delegate to — turning a C pointer and a `c_int`
count into a slice or `&str` — touches no FFI and *is* Miri-checkable, and that
is where undefined behaviour would live. `just miri` drives it with null
pointers, zero counts and negative counts.

`just asan` covers the other half: the trampolines actually invoked by Csound.
Doctests are excluded because they do not link under `-Zbuild-std` with a
sanitizer enabled.

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
