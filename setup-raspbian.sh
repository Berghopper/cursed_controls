#!/bin/sh

# TODO: add branch to download binary for armv6l, so we don't need to compile slowly.
# FIXME:
# - It seems that some libs/packages might intefere with getting nunchuck events.
#   If this is the case for you, try to only install what you see here.

sudo apt update -y && sudo apt upgrade -y

# Deps:
# xwiimote:
#  libtool autocon f automake m4 libudev-dev libncurses5-dev
# raw-gadget:
#  linux-headers-rpi-{v6,v7,v7l} vim
# this project:
#  clang
# misc:
#  git

sudo apt install -y git libtool autocon f automake m4 libudev-dev libncurses5-dev linux-headers-rpi-{v6,v7,v7l} vim clang

# wii udev rule + legacy bt
echo 'KERNEL=="uinput", MODE="0666"' | sudo tee -a /etc/udev/rules.d/wiimote.rules
sudo service udev restart
echo 'ClassicBondedOnly=false' | sudo tee -a /etc/bluetooth/input.conf
sudo service bluetooth restart

# Building xwiimote
cd ~
git clone https://github.com/xwiimote/xwiimote.git
cd xwiimote
./autogen.sh
make
sudo make install

# raw gadget...

echo "dtoverlay=dwc2" | sudo tee -a /boot/firmware/config.txt
echo "dwc2" | sudo tee -a /etc/modules

cd ~
git clone https://github.com/xairy/raw-gadget.git
cd raw-gadget/raw_gadget

make

# cursed controls

cd ~
git clone https://github.com/Berghopper/cursed_controls.git
cd cursed_controls

# First build 360 emulation module
git submodule sync
git submodule update --init --recursive
cd src/360-w-raw-gadget
make clean && make

# Get rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# build (for an hour on the pi0 :( ...)
cd ~/cursed_controlers
cargo build --release

echo "Done! You should reboot the system before running cursed_controls!"
