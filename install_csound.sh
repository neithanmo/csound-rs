#!/bin/sh
set -ex
git clone -b develop https://github.com/csound/csound.git
cd csound/
cmake . && make && sudo make install
sudo ldconfig
