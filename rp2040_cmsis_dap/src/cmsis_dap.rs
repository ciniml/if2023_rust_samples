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

use usb_device::{Result, control::RequestType};
use usb_device::bus::UsbBusAllocator;
use usb_device::class_prelude::*;
use usb_device::device::DEFAULT_ALTERNATE_SETTING;
use num_enum::{IntoPrimitive, TryFromPrimitive};

const USB_IF_CLASS_VENDOR: u8 = 0xff;
const USB_IF_SUBCLASS_VENDOR: u8 = 0x00;
const USB_IF_PROTOCOL_NONE: u8 = 0x00;

const BOS_CAPABILITY_TYPE_PLATFORM: u8 = 0x05;
const MS_OS_20_SET_HEADER_DESCRIPTOR: u16 = 0x0000;
const MS_OS_20_SUBSET_HEADER_CONFIGURATION: u16 = 0x0001;
const MS_OS_20_SUBSET_HEADER_FUNCTION: u16 = 0x0002;
const MS_OS_20_FEATURE_COMPATIBLE_ID: u16 = 0x0003;
const MS_OS_20_FEATURE_REG_PROPERTY: u16 = 0x0004;

#[repr(u16)]
#[derive(IntoPrimitive, TryFromPrimitive)]
enum RegPropertyType {
    Reserved,
    String,
    ExpandString,
    Binary,
    DwordLittleEndian,
    DwordBigEndian,
    Link,
    MultiString,
}

const MS_VENDOR_CODE: u8 = 0x01;

pub struct CmsisDapInterface<'a, B: UsbBus> {
    interface: InterfaceNumber,
    serial_string: StringIndex,
    out_ep: EndpointOut<'a, B>,
    in_ep: EndpointIn<'a, B>,
    response_buffer: [u8; 64],
    pending_response_bytes: Option<usize>,
}

impl<B: UsbBus> CmsisDapInterface<'_, B> {
    pub fn new(alloc: &UsbBusAllocator<B>, max_packet_size: u16) -> CmsisDapInterface<'_, B> {
        CmsisDapInterface {
            interface: alloc.interface(),       // インターフェース番号を確保
            serial_string: alloc.string(),      // インターフェース文字列の番号を確保
            out_ep: alloc.bulk(max_packet_size),    // Bulk OUT エンドポイントを確保
            in_ep: alloc.bulk(max_packet_size),     // Bulk IN エンドポイントを確保
            response_buffer: [0u8; 64],         // レスポンス格納用バッファ
            pending_response_bytes: None,       // 返信まちレスポンスバイト数 
        }
    }

    pub fn poll(&mut self) -> Result<()> {
        // 未送信レスポンスがあるか？
        if let Some(pending_response_bytes) = self.pending_response_bytes.as_ref() {
            self.in_ep.write(&self.response_buffer[..*pending_response_bytes])?;
            // 送信成功したのでクリア
            self.pending_response_bytes = None;
        }
        // コマンドを受信
        let mut response_length = 0;
        {
            let mut response = &mut self.response_buffer[..];
            // ホストからパケット受信
            let mut request_buffer = [0u8; 64];
            let request_length = self.out_ep.read(&mut request_buffer)?;
            let mut request = &request_buffer[..request_length];
            while request.len() > 0 {
                match request[0] {
                    0x00 => {   // DAP_Infoコマンド
                        if request.len() >= 2 {
                            // ID
                            let response_bytes = match request[1] {
                                0x01 => "vendor".as_bytes(),    // ベンダー名
                                0x02 => "product".as_bytes(),   // プロダクト名
                                0x03 => "serial".as_bytes(),    // シリアル番号
                                0x04 => "2.0.0".as_bytes(),     // CMSIS-DAPバージョン
                                0x09 => "1.0.0".as_bytes(),     // ファームウェアバージョン
                                0xf0 => &[0x01, 0x00],          // Capabilities = SWD
                                0xfe => &[0x01],                // 最大パケット数
                                0xff => &[64, 0],               // 最大パケットサイズ
                                _ => &[],                       // 未実装
                            };
                            // レスポンス・バッファに書き込み
                            response[response_length + 0] = 0;
                            response[response_length + 1] = response_bytes.len() as u8;
                            response[response_length + 2..response_length + 2 + response_bytes.len()]
                                .copy_from_slice(response_bytes);
                            let response_length_inc = 2 + response_bytes.len();
                            response_length += response_length_inc;
                            response = &mut response[response_length_inc..];
                            // リクエストの読み出し位置を更新
                            request = &request[2..];
                        }
                    },
                    _ => {
                        // 未実装コマンド。無視する
                        break;
                    },
                }
            }
        }
        
        if let Err(_) = self.in_ep.write(&self.response_buffer[..response_length]) {
            // 送信できなかったので送信まち状態とする
            self.pending_response_bytes = Some(response_length);
        }
        Ok(())
    }
}

impl<B: UsbBus> UsbClass<B> for CmsisDapInterface<'_, B> {
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
        writer.interface_alt(   // インターフェースディスクリプタを書き込み
            self.interface,     // インターフェース番号
            DEFAULT_ALTERNATE_SETTING,  // このコンフィグレーションのデフォルト・インターフェース
            USB_IF_CLASS_VENDOR,        // ベンダ固有クラス (0xff)
            USB_IF_SUBCLASS_VENDOR, // サブクラス (0x00)
            USB_IF_PROTOCOL_NONE,    // プロトコルなし (0x00)
            Some(self.serial_string),  // インターフェース文字列のインデックス 
        )?;
        writer.endpoint(&self.out_ep)?; // Bulk OUT エンドポイントディスクリプタを書き込み
        writer.endpoint(&self.in_ep)?;  // Bulk IN エンドポイントディスクリプタを書き込み

        Ok(())
    }
    fn get_string(&self, index: StringIndex, lang_id: u16) -> Option<&str> {
        let _ = lang_id;
        if index == self.serial_string {    // インターフェース文字列に対する要求？
            Some("CMSIS-DAP interface")     // インターフェース文字列を返す
        } else {
            None
        }
    }
    fn get_bos_descriptors(&self, writer: &mut BosWriter) -> Result<()> {
        #[rustfmt::skip]
        writer.capability(
            BOS_CAPABILITY_TYPE_PLATFORM,
            &[
                0,  // Reserved
                0xdf, 0x60, 0xdd, 0xd8,  // MS_OS_20_Platform_Capability_ID
                0x89, 0x45, 0xc7, 0x4c,  // {D8DD60DF-4589-4CC7-9CD2-659D9E648A9F}
                0x9c, 0xd2, 0x65, 0x9d,  // 
                0x9e, 0x64, 0x8a, 0x9f,  //
                0x00, 0x00, 0x03, 0x06,  // dwWindowsVersion – 0x06030000 (Win8.1 or later)
                174, 0,                  // wLength = MS OS 2.0 descriptor set
                MS_VENDOR_CODE,          // bMS_VendorCode
                0x00,                    // bAltEnumCmd - does not support alternate enum.
            ]
        )?;
        Ok(())
    }

    fn control_in(&mut self, xfer: ControlIn<B>) {
        let request = xfer.request();
        if request.request_type == RequestType::Vendor
            && request.request == MS_VENDOR_CODE
            && request.value == 0
            && request.index == 7
        {
            // Request to retrieve MS OS 2.0 Descriptor Set.
            let interface_number = self.interface;
            xfer.accept(|buffer| {
                write_descriptor_set(buffer, 0x06030000, |buffer| {
                    write_configuration_subset(buffer, |buffer| {
                        write_function_subset(buffer, interface_number, |buffer| {
                            let mut offset = 0;
                            // Registry property name and value must be null-terminated UTF-16 string so we have to allocate double size of the original string and fill it with 0 padded on MSB.
                            let mut property_name = [0u8; b"DeviceInterfaceGUID\0".len() * 2];
                            fill_utf16(&mut property_name, b"DeviceInterfaceGUID\0");
                            let mut property_data =
                                [0u8; b"{A5DCBF10-6530-11D2-901F-00C04FB951ED}\0".len() * 2];
                            fill_utf16(
                                &mut property_data,
                                b"{A5DCBF10-6530-11D2-901F-00C04FB951ED}\0",
                            );

                            // Set Compatible ID to WINUSB in order to be WinUSB driver is loaded for this interface.
                            offset += write_compatible_id(
                                &mut buffer[offset..],
                                b"WINUSB\0\0",
                                &[0u8; 8],
                            )?;
                            // Set GUID_DEVINTERFACE_USB_DEVICE({A5DCBF10-6530-11D2-901F-00C04FB951ED}) Device Interface Class to be enumerated by libusb.
                            offset += write_registry_property(
                                &mut buffer[offset..],
                                &property_name,
                                RegPropertyType::String,
                                &property_data,
                            )?;
                            Ok(offset)
                        })
                    })
                })
            })
            .unwrap();
        }
    }
}


fn write_descriptor_set(
    buffer: &mut [u8],
    windows_version: u32,
    f: impl FnOnce(&mut [u8]) -> Result<usize>,
) -> Result<usize> {
    let length = 10usize;
    if buffer.len() < length {
        return Err(UsbError::BufferOverflow);
    }
    buffer[0] = u16_lo(length as u16);
    buffer[1] = u16_hi(length as u16);
    buffer[2] = u16_lo(MS_OS_20_SET_HEADER_DESCRIPTOR);
    buffer[3] = u16_hi(MS_OS_20_SET_HEADER_DESCRIPTOR);
    buffer[4] = u16_lo(u32_lo(windows_version));
    buffer[5] = u16_hi(u32_lo(windows_version));
    buffer[6] = u16_lo(u32_hi(windows_version));
    buffer[7] = u16_hi(u32_hi(windows_version));
    let total_length = f(&mut buffer[length..])? + length;
    buffer[8] = u16_lo(total_length as u16);
    buffer[9] = u16_hi(total_length as u16);
    Ok(total_length)
}

fn write_configuration_subset(
    buffer: &mut [u8],
    f: impl FnOnce(&mut [u8]) -> Result<usize>,
) -> Result<usize> {
    let length = 8usize;
    if buffer.len() < length {
        return Err(UsbError::BufferOverflow);
    }
    buffer[0] = u16_lo(length as u16);
    buffer[1] = u16_hi(length as u16);
    buffer[2] = u16_lo(MS_OS_20_SUBSET_HEADER_CONFIGURATION);
    buffer[3] = u16_hi(MS_OS_20_SUBSET_HEADER_CONFIGURATION);
    buffer[4] = 0; // Currently usb_device supports one configuration.
    buffer[5] = 0; // reserved
    let total_length = f(&mut buffer[length..])? + length;
    buffer[6] = u16_lo(total_length as u16);
    buffer[7] = u16_hi(total_length as u16);
    Ok(total_length)
}

fn write_function_subset(
    buffer: &mut [u8],
    first_interface_number: InterfaceNumber,
    f: impl FnOnce(&mut [u8]) -> Result<usize>,
) -> Result<usize> {
    let length = 8usize;
    if buffer.len() < length {
        return Err(UsbError::BufferOverflow);
    }
    buffer[0] = u16_lo(length as u16);
    buffer[1] = u16_hi(length as u16);
    buffer[2] = u16_lo(MS_OS_20_SUBSET_HEADER_FUNCTION);
    buffer[3] = u16_hi(MS_OS_20_SUBSET_HEADER_FUNCTION);
    buffer[4] = first_interface_number.into();
    buffer[5] = 0; // reserved
    let total_length = f(&mut buffer[length..])? + length;
    buffer[6] = u16_lo(total_length as u16);
    buffer[7] = u16_hi(total_length as u16);
    Ok(total_length)
}

fn write_compatible_id(
    buffer: &mut [u8],
    compatible_id: &[u8; 8],
    sub_compatible_id: &[u8; 8],
) -> Result<usize> {
    let length = 20usize;
    if buffer.len() < length {
        return Err(UsbError::BufferOverflow);
    }
    buffer[0] = u16_lo(length as u16);
    buffer[1] = u16_hi(length as u16);
    buffer[2] = u16_lo(MS_OS_20_FEATURE_COMPATIBLE_ID);
    buffer[3] = u16_hi(MS_OS_20_FEATURE_COMPATIBLE_ID);
    buffer[4..12].copy_from_slice(compatible_id);
    buffer[12..20].copy_from_slice(sub_compatible_id);
    Ok(length)
}

fn write_registry_property(
    buffer: &mut [u8],
    property_name: &[u8],
    property_type: RegPropertyType,
    property_data: &[u8],
) -> Result<usize> {
    let name_len = property_name.len();
    let data_len = property_data.len();
    let length = name_len + data_len + 10;
    if buffer.len() < length {
        return Err(UsbError::BufferOverflow);
    }
    let property_type: u16 = property_type.into();
    buffer[0] = u16_lo(length as u16);
    buffer[1] = u16_hi(length as u16);
    buffer[2] = u16_lo(MS_OS_20_FEATURE_REG_PROPERTY);
    buffer[3] = u16_hi(MS_OS_20_FEATURE_REG_PROPERTY);
    buffer[4..6].copy_from_slice(&property_type.to_le_bytes());
    buffer[6..8].copy_from_slice(&(name_len as u16).to_le_bytes());
    buffer[8..8 + name_len].copy_from_slice(property_name);
    buffer[8 + name_len..8 + name_len + 2].copy_from_slice(&(data_len as u16).to_le_bytes());
    buffer[8 + name_len + 2..8 + name_len + 2 + data_len].copy_from_slice(property_data);

    Ok(length)
}

fn fill_utf16<const N: usize>(buf: &mut [u8], b: &[u8; N]) {
    for i in 0..N {
        buf[i * 2] = b[i];
        buf[i * 2 + 1] = 0;
    }
}

const fn u16_lo(v: u16) -> u8 {
    (v & 0xff) as u8
}
const fn u16_hi(v: u16) -> u8 {
    (v >> 8) as u8
}
const fn u32_lo(v: u32) -> u16 {
    (v & 0xfffff) as u16
}
const fn u32_hi(v: u32) -> u16 {
    (v >> 16) as u16
}
