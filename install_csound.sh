#!/bin/sh
set -ex
git clone https://github.com/csound/csound.git
cd csound/
cmake . && make && sudo make install
ldconfig
