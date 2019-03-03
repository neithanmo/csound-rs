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
1. [License](#license)
1. [Contribution](#contribution)

<a name="installation"/>

## Installation

To build the Csound bindings or anything depending on this crate, you need to
have at least Csound 6.11, previous version of Csound are not suported.
By default( The only supported way), this crate will attempt to dynamically link to the system-wide libcsound64.

<a name="installation-linux"/>

### Linux/BSDs

You need to install Csound with your distributions
package manager, or in case your package manager has a unsupported version of Csound( <6.11 ) you have to build it from source.

On Debian/Ubuntu Csound can be installed with

```
# Make sure the version of this package is >= 6.11
$ apt-get install libcsound64-6.0 libcsound64-dev
```

Also, You can compile it from source and install(recommended)

```
# First, install all the csound's dependencies
$ apt-get install build-essential libportaudio2 portaudio19-dev cmake //
lib64ncurses5-dev lib64ncurses5 flex bison libsndfile1-dev libsndfile1
```
then, clone the csound's source code
```
# Clone Csound from its repository
$ git clone https://github.com/csound/csound.git
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

<a name="installation-macos"/>

### macOS

Please be free to send a pull request with the changes applied to the build
scripts and instructions about how to use this crate along csound's native library

<a name="installation-windows"/>

### Windows

Download the csound's installer for [*windows*](https://github.com/csound/csound/releases/download/6.12.2/Csound6.12.0-Windows_x64-installer.exe)
Follow the instalation steps. 
1. Locate your csound installation directory ( commonly it is *C:\\Program Files\\Csound6_x64*)
2. Open Command Prompt (make sure you Run as administrator so you're able to add a system environment variable).
3. Set the environment variable as follows:
```
$ setx CSOUND_LIB_DIR "C:\\Program Files\\Csound6_x64\\lib"
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
For running the examples 1 to 5 just:
```
# Runs the example 5
$ cargo --release --example example5
```
The anothers examples requires some dependencies, but you can run them through calling cargo on their own Cargo.toml file
```
# Runs the example 5
$ cd examples/example9
$ cargo --release build
$ cargo run
```
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
