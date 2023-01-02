// Copyright 2023 Kenta Ida
//
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![no_std]
#![no_main]

use hal::pac;
use panic_halt as _;
use rp_pico::hal;

use usb_device::bus::UsbBusAllocator;
use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

#[rp_pico::hal::entry]
fn main() -> ! {
    let pac = pac::Peripherals::take().unwrap();

    let mut resets = pac.RESETS;

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut resets,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let usb_allocator = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut resets,
    ));

    let mut usb_serial = SerialPort::new(&usb_allocator);
    let mut usb_device = UsbDeviceBuilder::new(&usb_allocator, UsbVidPid(0x6666, 0x4444))
        .manufacturer("test manufacturer")
        .product("test product")
        .serial_number("serial number")
        .device_class(USB_CLASS_CDC)
        .composite_with_iads()
        .max_packet_size_0(64)
        .build();

    let mut buffer = [0u8; 64];
    let mut pending_bytes_to_write = None;
    loop {
        // Loopback serial data
        if pending_bytes_to_write.is_none() {
            if let Ok(bytes_read) = usb_serial.read(&mut buffer) {
                pending_bytes_to_write = Some((0, bytes_read))
            }
        }
        if let Some((bytes_written, bytes_to_write)) = pending_bytes_to_write {
            if let Ok(bytes_written_now) = usb_serial.write(&buffer[bytes_written..bytes_to_write])
            {
                let bytes_written = bytes_written + bytes_written_now;
                if bytes_written == bytes_to_write {
                    pending_bytes_to_write = None;
                } else {
                    pending_bytes_to_write = Some((bytes_written, bytes_to_write));
                }
            }
        }
        usb_device.poll(&mut [&mut usb_serial]);
    }
}
