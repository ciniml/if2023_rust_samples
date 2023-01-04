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

mod cmsis_dap;
use cmsis_dap::CmsisDapInterface;

use hal::pac;
use panic_halt as _;
use rp_pico::hal;

use usb_device::bus::UsbBusAllocator;
use usb_device::prelude::*;

#[rp_pico::hal::entry]
fn main() -> ! {
    let pac = pac::Peripherals::take().unwrap();

    let mut resets = pac.RESETS;

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    // クロックを初期化
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
    // UsbBusを初期化
    let usb_bus = hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,   // RP2040のUSBペリフェラルのレジスタ
        pac.USBCTRL_DPRAM,  // RP2040のUSBペリフェラルのDPRAM
        clocks.usb_clock,   // USBクロック
        true,               // Vbus検出ビットを強制的にセットする
        &mut resets,        // サブシステムのリセット・レジスタ
    );
    const MAX_PACKET_SIZE: u8 = 64;
    // UsbBusAllocatorを構築
    // ※UsbBusAllocatorは内部可変性を持つ型なのでmutでなくて良い
    let usb_bus_allocator = UsbBusAllocator::new(usb_bus);
    // usb-serialクレートのSerialPortを構築
    let mut cmsis_dap = CmsisDapInterface::new(&usb_bus_allocator, MAX_PACKET_SIZE as u16);
    // UsbDeviceを構築 VID=0x6666, PID=0x4444 (prototype product)
    let mut usb_device = UsbDeviceBuilder::new(&usb_bus_allocator, UsbVidPid(0x6666, 0x4444))
        .manufacturer("test manufacturer")  // Manufacturer  = "test manufacturer"
        .product("test product")            // Product       = "test product"
        .serial_number("serial number")     // Serial Number = "serial number" 
        .composite_with_iads()              // IADを使った複合デバイスとする
        .max_packet_size_0(MAX_PACKET_SIZE) // 最大パケットサイズ (64バイト)
        .build();                           // 上記の設定でUsbDeviceを構築

    loop {
        // USBデバイスのイベントなどを処理する
        usb_device.poll(&mut [&mut cmsis_dap]);
        // CMSIS-DAPのコマンドを処理する
        cmsis_dap.poll().ok();
    }
}
