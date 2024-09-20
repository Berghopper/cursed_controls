# Raspbian setup

Tested on Raspberry Pi Zero / Raspberry Pi Zero 2

Before proceeding, make sure you're using a 32-bit Raspberry Pi OS image (if using the pi zero/zero 2). This is mainly to avoid slow builds because of HW limitations.
Make sure the system is up to date:

```bash
sudo apt update -y && sudo apt upgrade -y
```

And install the required dependencies:

If using a 32-bit OS:

```bash
sudo apt install -y git libtool autoconf automake m4 libudev-dev libncurses5-dev vim clang linux-headers-rpi-{v6,v7,v7l}
```

64-bit OS, e.g. Raspberry Pi 4:

```bash
sudo apt install -y git libtool autoconf automake m4 libudev-dev libncurses5-dev vim clang linux-headers-rpi-v8
```

details:

```bash
# xwiimote depends on:
#  libtool autoconf automake m4 libudev-dev libncurses5-dev
# raw-gadget depends on:
#  linux-headers-rpi-{v6,v7,v7l} vim
# this project depends on:
#  clang
# misc:
#  git
```

Add a udev rule for wiimote input reading:

```bash
echo 'KERNEL=="uinput", MODE="0666"' | sudo tee -a /etc/udev/rules.d/wiimote.rules
sudo service udev restart
```

Edit bluez config to use `ClassicBondedOnly=false`, which helps keep wii motes connected.

```bash
echo 'ClassicBondedOnly=false' | sudo tee -a /etc/bluetooth/input.conf
sudo service bluetooth restart
```

We could install `xwiimote-lib` as a system package, but I've found this does not work reliably for whatever reason.
Building it from scratch works just fine however:

```bash
cd ~
git clone https://github.com/xwiimote/xwiimote.git
cd xwiimote
./autogen.sh
make -j
sudo make install
```

Now we need to enable the Pi's OTG functionality and install `raw-gadget` so we can [fake an xbox controller as our output.](https://github.com/Berghopper/360-w-raw-gadget)

```bash
echo "dtoverlay=dwc2" | sudo tee -a /boot/firmware/config.txt
echo "dwc2" | sudo tee -a /etc/modules

cd ~
git clone https://github.com/xairy/raw-gadget.git
cd ~/raw-gadget/raw_gadget
make -j
```

Now for this project:

```bash
cd ~
git clone https://github.com/Berghopper/cursed_controls.git
cd cursed_controls

# First build 360 emulation module
git submodule sync
git submodule update --init --recursive
cd src/360-w-raw-gadget
```

Now depending on your hardware, refer to [360-w-raw-gadget](https://github.com/Berghopper/360-w-raw-gadget)'s table for making the projet.

e.g. pi zero:
```bash
make clean && make -j
```
or pi zero 2:
```bash
make clean && make -j rpi0_2
```
continue:
```bash
# Get rust
# First set variable for 1 cpu only, because of low memory.
# see; https://github.com/rust-lang/rustup/issues/2919
export RUSTUP_IO_THREADS=1
env RUSTUP_IO_THREADS=1 curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# build (for an hour on the pi0/20 minutes on pi0 v2. should perhaps cross-compile/add binary releases...)
cd ~/cursed_controlers
cargo build --release
```

At this point, if you haven't yet. Reboot.

After Reboot, you can run the `init-raspbian.sh` script from the repo

```bash
cd ~/cursed_controls
./init-raspbian.sh
```

Or alternatively:

```bash
sudo modprobe uinput
sudo modprobe hid-wiimote
cd ~/raw-gadget/raw_gadget/
sudo ./insmod.sh
```

To run cursed controls:

```bash
cd ~/cursed_controls
sudo ./target/release/cursed_controls
```

## Known issues

Some libraries/pacakges might interfere with `xwiimote`, to ensure this is the case, try an install from scratch following this guide.

## Power issues over USB

For the Raspberry Pi zero 2, you might find the following config helpful to save energy (if running in headless mode):

`/boot/firmware/config.txt`:

```toml
[all]
# stable OC
#arm_freq=1300
#over_voltage=6

dtoverlay=dwc2

# Power save
# Enable Bluetooth and WiFi
dtoverlay=disable-bt=off
dtoverlay=disable-wifi=off

# Disable HDMI to save power
hdmi_blanking=1
hdmi_ignore_hotplug=1

# Disable the camera and display auto-detect to save power
camera_auto_detect=0
display_auto_detect=0

# Audio is not typically needed for headless or low-power setups
dtparam=audio=off

# Reduce GPU memory to the minimum
gpu_mem=16

# Disable LEDs to reduce power consumption
dtparam=act_led_trigger=none
dtparam=act_led_activelow=on
```
