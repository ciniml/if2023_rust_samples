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

use core::cell::RefCell;

use hal::pac;
use panic_halt as _;
use rp_pico::hal;

use hal::usb::UsbBus;
use usb_device::bus::UsbBusAllocator;
use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};
use hal::pac::interrupt;

use cortex_m::interrupt::Mutex;

static mut USB_BUS_ALLOCATOR: Option<UsbBusAllocator<hal::usb::UsbBus>> = None;
static USB_DEVICE: Mutex<RefCell<Option<UsbDevice<'static, UsbBus>>>>= Mutex::new(RefCell::new(None));
static USB_SERIAL: Mutex<RefCell<Option<SerialPort<'static, UsbBus>>>> = Mutex::new(RefCell::new(None));

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
    unsafe { USB_BUS_ALLOCATOR = Some(usb_allocator); }

    let usb_allocator = unsafe { USB_BUS_ALLOCATOR.as_ref().unwrap() };
    let usb_serial = SerialPort::new(usb_allocator);
    let usb_device = UsbDeviceBuilder::new(usb_allocator, UsbVidPid(0x6666, 0x4444))
        .manufacturer("test manufacturer")
        .product("test product")
        .serial_number("serial number")
        .device_class(USB_CLASS_CDC)
        .composite_with_iads()
        .max_packet_size_0(64)
        .build();
    
    cortex_m::interrupt::free(|cs| {
        *USB_DEVICE.borrow(cs).borrow_mut() = Some(usb_device);
        *USB_SERIAL.borrow(cs).borrow_mut() = Some(usb_serial);
    });

    // unsafe {
    //     pac::NVIC::unmask(pac::Interrupt::USBCTRL_IRQ);
    //     cortex_m::interrupt::enable();
    // }

    let mut buffer = [0u8; 64];
    let mut pending_bytes_to_write = None;
    loop {
        // Loopback serial data
        cortex_m::interrupt::free(|cs| {
            let usb_serial = USB_SERIAL.borrow(cs);
            if pending_bytes_to_write.is_none() {
                if let Ok(bytes_read) = usb_serial.borrow_mut().as_mut().unwrap().read(&mut buffer) {
                    pending_bytes_to_write = Some((0, bytes_read))
                }
            }
            if let Some((bytes_written, bytes_to_write)) = pending_bytes_to_write {
                if let Ok(bytes_written_now) = usb_serial.borrow_mut().as_mut().unwrap().write(&buffer[bytes_written..bytes_to_write]) {
                    let bytes_written = bytes_written + bytes_written_now;
                    if bytes_written == bytes_to_write {
                        pending_bytes_to_write = None;
                    } else {
                        pending_bytes_to_write = Some((bytes_written, bytes_to_write));
                    }
                }
            }
        });
        //cortex_m::asm::wfi();
        let _poll_result = cortex_m::interrupt::free(|cs|{
            let usb_serial = USB_SERIAL.borrow(cs);
            let usb_device = USB_DEVICE.borrow(cs);
            usb_device.borrow_mut().as_mut().unwrap().poll(&mut [usb_serial.borrow_mut().as_mut().unwrap()])
        });
    }

}

#[interrupt]
fn USBCTRL_IRQ() { 
    let _poll_result = cortex_m::interrupt::free(|cs|{
        let usb_serial = USB_SERIAL.borrow(cs);
        let usb_device = USB_DEVICE.borrow(cs);
        usb_device.borrow_mut().as_mut().unwrap().poll(&mut [usb_serial.borrow_mut().as_mut().unwrap()])
    });
}
