# csound

This is a **Rust** bindings for **Csound**.
[Csound](https://csound.com/) bindings for Rust.
Documentation can be found [here](https://slomo.pages.freedesktop.org/rustdocs/gstreamer/gstreamer_app/).

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

Also, You can compile it from source and install
```
# Clone Csound from its repository
$ git clone https://github.com/csound/csound.git
```
And follow the installation instructions indicated in the BUILD.md file, which can be found inside
of the csound repository.

<a name="installation-macos"/>

### macOS

Please be free to send a pull request with the changes applied to the build
scripts and instructions about how to use this crate along csound's native library

<a name="installation-windows"/>

### Windows

Again, please be free to send a pull request with the changes applied to the build
scripts and instructions about how to use this crate along csound's native library

<a name="getting-started"/>

## Getting Started

The API reference can be found
[here](https://csound.com/docs/api/index.html)

For getting started withCsound-rs, you have to understand some basic concepts about Csound, before to try to use this
bindigs. Please check the Get Started page in the Csound's site
[Get Started](https://csound.com/get-started.html)
In addition there are csound api [examples](https://github.com/csound/csoundAPI_examples) inside of the rust directory.

<a name="license"/>

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
