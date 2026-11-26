//! Steam Controller over raw HID, without Steam.
//!
//! The 2026 Steam Controller ("Triton"/IBEX, product ids 0x1302 wired,
//! 0x1303 Bluetooth, 0x1304 dongle) speaks Valve's own HID protocol, not
//! XInput, so the title never sees it through `XInputGetState`. This module
//! reads the controller directly: it disables the firmware's keyboard/mouse
//! emulation ("lizard mode") with a feature report and decodes the state
//! packets into the same shape [`super::dualsense`] produces.
//!
//! It works with Steam running: HID handles are shared, and the feature
//! report is re-sent every few seconds in case Steam has changed the mode
//! again. A Steam-created virtual XInput pad is ignored because the gameinput
//! host source asks this module first (see `gameinput::host::pad`).
//!
//! Protocol: Valve's `controller_structs.h` / `controller_constants.h` as
//! carried by SDL (`SDL_hidapi_steam_triton.c`, zlib licence). Only the
//! fields needed for an Xbox pad are decoded; touchpads, IMU and haptics are
//! not reported.

#![allow(non_snake_case)]

use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    GUID_DEVINTERFACE_HID, HIDD_ATTRIBUTES, HIDP_CAPS, HIDP_STATUS_SUCCESS, HidD_FreePreparsedData,
    HidD_GetAttributes, HidD_GetPreparsedData, HidD_SetFeature, HidP_GetCaps,
};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile,
};

/// One pad state, as the HID reader keeps it (the shape gameinput reads).
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Reading {
    pub buttons: u32,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub left_x: f32,
    pub left_y: f32,
    pub right_x: f32,
    pub right_y: f32,
}

/// Pad buttons, the bits the title maps (same values as `gameinput`).
const MENU: u32 = 0x1;
const VIEW: u32 = 0x2;
const A: u32 = 0x4;
const B: u32 = 0x8;
const X: u32 = 0x10;
const Y: u32 = 0x20;
const UP: u32 = 0x40;
const DOWN: u32 = 0x80;
const LEFT: u32 = 0x100;
const RIGHT: u32 = 0x200;
const LEFT_SHOULDER: u32 = 0x400;
const RIGHT_SHOULDER: u32 = 0x800;
const LEFT_STICK: u32 = 0x1000;
const RIGHT_STICK: u32 = 0x2000;

/// Valve's vendor id and the 2026 controller's product ids.
const VALVE: u16 = 0x28de;
const CONTROLLERS: [u16; 3] = [
    0x1302, // 2026 Steam Controller, wired
    0x1303, // 2026 Steam Controller, Bluetooth
    0x1304, // 2026 Steam Controller, dongle
];

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;

/// Report ids: a state packet, the BLE variant and the 16-bit-timestamp
/// variant. All three start with the same 17 bytes.
const STATE: u8 = 0x42;
const STATE_BLE: u8 = 0x45;
const STATE_TIMESTAMP: u8 = 0x47;

/// Triton button bits (`TRITON_*` in SDL's `SDL_hidapi_steam_triton.c`).
const T_A: u32 = 0x0000_0001;
const T_B: u32 = 0x0000_0002;
const T_X: u32 = 0x0000_0004;
const T_Y: u32 = 0x0000_0008;
const T_QAM: u32 = 0x0000_0010;
const T_R3: u32 = 0x0000_0020;
const T_VIEW: u32 = 0x0000_0040;
const T_MENU: u32 = 0x0000_4000;
const T_L3: u32 = 0x0000_8000;
const T_R: u32 = 0x0000_0200;
const T_L: u32 = 0x0008_0000;
const T_DPAD_DOWN: u32 = 0x0000_0400;
const T_DPAD_RIGHT: u32 = 0x0000_0800;
const T_DPAD_LEFT: u32 = 0x0000_1000;
const T_DPAD_UP: u32 = 0x0000_2000;
const T_STEAM: u32 = 0x0001_0000;

/// The feature report that turns lizard mode off: report 1, message 0x87
/// (`ID_SET_SETTINGS_VALUES`), one 3-byte setting, `SETTING_LIZARD_MODE` = 9,
/// value `LIZARD_MODE_OFF` = 0.
const LIZARD_OFF: [u8; 6] = [0x01, 0x87, 0x03, 0x09, 0x00, 0x00];

static READING: Mutex<Option<Reading>> = Mutex::new(None);
static START: Once = Once::new();

/// The controller's newest state, starting the reader on first use.
pub fn latest() -> Option<Reading> {
    if !enabled() {
        return None;
    }
    START.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("steam-controller".into())
            .spawn(reader);
    });
    *READING.lock().unwrap_or_else(|e| e.into_inner())
}

/// `VICTORY_STEAM=0` turns the direct reader off.
fn enabled() -> bool {
    std::env::var_os("VICTORY_STEAM").is_none_or(|v| v != "0")
}

/// An open controller: its input handle, the HID description of it (freed on
/// drop), the input report length and the feature report length.
struct Pad {
    handle: HANDLE,
    preparsed: isize,
    input_len: usize,
    feature_len: usize,
    writable: bool,
}

impl Drop for Pad {
    fn drop(&mut self) {
        // SAFETY: both came from the HID API for this handle.
        unsafe {
            HidD_FreePreparsedData(self.preparsed);
            CloseHandle(self.handle);
        }
    }
}

// SAFETY: a HID handle may be used from any thread, and each Pad is owned by
// exactly one reader thread.
unsafe impl Send for Pad {}

/// Interfaces already being read, by device path: one thread each, so a
/// controller that is switched on later is opened without restarting the
/// threads that are blocked in `ReadFile` on a silent dongle interface.
static OPEN: Mutex<Vec<Vec<u16>>> = Mutex::new(Vec::new());

fn reader() {
    loop {
        let paths = device_paths();
        let mut opened = 0;
        for path in paths {
            {
                let open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
                if open.iter().any(|known| known == &path) {
                    opened += 1;
                    continue;
                }
            }
            if let Some(pad) = open_path(&path) {
                OPEN.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(path.clone());
                let key = path.clone();
                let spawned = std::thread::Builder::new()
                    .name("steam-pad".into())
                    .spawn(move || {
                        read_reports(&pad);
                        let mut open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
                        open.retain(|known| known != &key);
                    });
                if spawned.is_ok() {
                    opened += 1;
                } else {
                    let mut open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
                    open.retain(|known| known != &path);
                }
            }
        }
        if opened == 0 {
            *READING.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        // Off, out of range or in a different mode: look for it again.
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Reads until the controller goes away. Lizard mode is turned off again
/// every few seconds while Steam may be writing its own settings.
fn read_reports(pad: &Pad) {
    let mut buf = vec![0u8; pad.input_len.max(64)];
    let log = std::env::var_os("VICTORY_PAD_LOG").is_some();
    let dump = std::env::var_os("VICTORY_STEAM_DUMP").is_some();
    let mut last: Option<Reading> = None;
    let mut maintenance = Instant::now() - Duration::from_secs(3);
    loop {
        if maintenance.elapsed() >= Duration::from_secs(3) {
            lizard_off(pad);
            maintenance = Instant::now();
        }
        let mut read = 0u32;
        // SAFETY: a live handle and a buffer of its report length.
        let ok = unsafe {
            ReadFile(
                pad.handle,
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || read == 0 {
            return;
        }
        let report = &buf[..read as usize];
        if dump {
            let end = report.len().min(24);
            tracing::info!(
                hex = %report[..end].iter().map(|b| format!("{b:02x}")).collect::<String>(),
                "Steam Controller report"
            );
        }
        let Some(reading) = parse(report) else {
            continue;
        };
        if last == Some(reading) {
            continue;
        }
        if log && last.is_none_or(|before| before.buttons != reading.buttons) {
            tracing::info!(
                buttons = format_args!("{:#x}", reading.buttons),
                left = format_args!("{:.2},{:.2}", reading.left_x, reading.left_y),
                right = format_args!("{:.2},{:.2}", reading.right_x, reading.right_y),
                triggers = format_args!("{:.2},{:.2}", reading.left_trigger, reading.right_trigger),
                "Steam Controller"
            );
        }
        last = Some(reading);
        *READING.lock().unwrap_or_else(|e| e.into_inner()) = Some(reading);
    }
}

/// Sends the lizard-mode-off feature report, if the handle allows it.
fn lizard_off(pad: &Pad) {
    if !pad.writable {
        return;
    }
    let mut buf = vec![0u8; pad.feature_len.max(LIZARD_OFF.len())];
    buf[..LIZARD_OFF.len()].copy_from_slice(&LIZARD_OFF);
    // SAFETY: a live handle, a report id in byte 0 and a buffer it can read.
    unsafe { HidD_SetFeature(pad.handle, buf.as_ptr().cast(), buf.len() as u32) };
}

/// Opens one Valve controller interface and keeps its HID description.
fn open_path(path: &[u16]) -> Option<Pad> {
    // Write access is needed for the lizard-mode feature report; if Steam
    // or another program holds the interface exclusively, read-only still
    // decodes the state packets.
    for write in [true, false] {
            let access = if write {
                GENERIC_READ | GENERIC_WRITE
            } else {
                GENERIC_READ
            };
            // SAFETY: a path from SetupAPI, opened shared as HID requires.
            let handle = unsafe {
                CreateFileW(
                    path.as_ptr(),
                    access,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                continue;
            }
            // SAFETY: a plain Windows struct for which all zeroes is valid.
            let mut attributes: HIDD_ATTRIBUTES = unsafe { std::mem::zeroed() };
            attributes.Size = std::mem::size_of::<HIDD_ATTRIBUTES>() as u32;
            // SAFETY: a live HID handle and room for the attributes.
            let known = unsafe { HidD_GetAttributes(handle, &mut attributes) };
            if !known || attributes.VendorID != VALVE || !CONTROLLERS.contains(&attributes.ProductID)
            {
                // SAFETY: the handle came from CreateFileW above and was not kept.
                unsafe { CloseHandle(handle) };
                continue;
            }
            let mut preparsed: isize = 0;
            // SAFETY: a live HID handle and room for the pointer.
            if !unsafe { HidD_GetPreparsedData(handle, &mut preparsed) } {
                // SAFETY: the handle came from CreateFileW above.
                unsafe { CloseHandle(handle) };
                continue;
            }
            // SAFETY: a plain Windows struct for which all zeroes is valid.
            let mut caps: HIDP_CAPS = unsafe { std::mem::zeroed() };
            // SAFETY: the preparations are ours until freed.
            let status = unsafe { HidP_GetCaps(preparsed, &mut caps) };
            let input_len = if status == HIDP_STATUS_SUCCESS {
                usize::from(caps.InputReportByteLength).max(1)
            } else {
                64
            };
            let feature_len = if status == HIDP_STATUS_SUCCESS {
                usize::from(caps.FeatureReportByteLength).max(LIZARD_OFF.len())
            } else {
                64
            };
            // A state packet is 17 bytes; interfaces shorter than that are not
            // the controller's input collection.
            if input_len < 17 {
                // SAFETY: both came from the HID API for this handle.
                unsafe {
                    HidD_FreePreparsedData(preparsed);
                    CloseHandle(handle);
                }
                continue;
            }
            tracing::info!(
                product = format_args!("{:#06x}", attributes.ProductID),
                report = input_len,
                writable = write,
                "Steam Controller interface opened over HID"
            );
            let pad = Pad {
                handle,
                preparsed,
                input_len,
                feature_len,
                writable: write,
            };
            lizard_off(&pad);
            return Some(pad);
    }
    None
}

/// Every present HID interface's device path.
fn device_paths() -> Vec<Vec<u16>> {
    let mut paths = Vec::new();
    // SAFETY: the HID interface class, no enumerator or parent window.
    let set: HDEVINFO = unsafe {
        SetupDiGetClassDevsW(
            &GUID_DEVINTERFACE_HID,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    if set == -1 {
        return paths;
    }
    let mut index = 0u32;
    loop {
        // SAFETY: a plain Windows struct for which all zeroes is valid.
        let mut interface: SP_DEVICE_INTERFACE_DATA = unsafe { std::mem::zeroed() };
        interface.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
        // SAFETY: the set from above, and room for one interface.
        if unsafe {
            SetupDiEnumDeviceInterfaces(
                set,
                std::ptr::null(),
                &GUID_DEVINTERFACE_HID,
                index,
                &mut interface,
            )
        } == 0
        {
            break;
        }
        index += 1;
        let mut required = 0u32;
        // SAFETY: asks for the room the detail needs.
        unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                std::ptr::null_mut(),
                0,
                &mut required,
                std::ptr::null_mut(),
            )
        };
        if required == 0 {
            continue;
        }
        let mut buffer = vec![0u8; required as usize];
        let detail = buffer
            .as_mut_ptr()
            .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        // SAFETY: the buffer is the size the API asked for; cbSize is its header.
        unsafe {
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                detail,
                required,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0
            {
                continue;
            }
        }
        // The path starts after the u32 cbSize, as UTF-16.
        // SAFETY: the API wrote the path at this offset in our buffer.
        let path = unsafe { buffer.as_ptr().add(4).cast::<u16>() };
        let mut len = 0usize;
        // SAFETY: the API wrote a NUL-terminated path into the buffer.
        while unsafe { *path.add(len) } != 0 {
            len += 1;
        }
        // With its NUL: `CreateFileW` reads up to it, and without one it read
        // past the end of the copy and stopped the run (as DualSense did; same
        // fault: ntdll RtlInitUnicodeStringEx under KERNELBASE CreateFileW).
        // SAFETY: `path` is valid for `len` u16s in the live buffer.
        let mut owned = unsafe { std::slice::from_raw_parts(path, len) }.to_vec();
        owned.push(0);
        paths.push(owned);
    }
    // SAFETY: the set is ours.
    unsafe { SetupDiDestroyDeviceInfoList(set) };
    paths
}

/// One state packet (`data[0]` is the report id) as a pad reading.
fn parse(report: &[u8]) -> Option<Reading> {
    let (&id, payload) = report.split_first()?;
    if !matches!(id, STATE | STATE_BLE | STATE_TIMESTAMP) || payload.len() < 17 {
        return None;
    }
    // Packed TritonMTUNoQuat_t / TritonMTUNoQuat32TS_t header:
    // seq u8, buttons u32, triggers i16 x2, sticks i16 x4.
    let buttons = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
    let trigger = |at: usize| {
        let raw = i16::from_le_bytes([payload[at], payload[at + 1]]);
        (f32::from(raw) / 32767.0).clamp(0.0, 1.0)
    };
    let axis = |at: usize| {
        let raw = i16::from_le_bytes([payload[at], payload[at + 1]]);
        (f32::from(raw) / 32767.0).clamp(-1.0, 1.0)
    };
    let bits = [
        (T_A, A),
        (T_B, B),
        (T_X, X),
        (T_Y, Y),
        (T_DPAD_UP, UP),
        (T_DPAD_DOWN, DOWN),
        (T_DPAD_LEFT, LEFT),
        (T_DPAD_RIGHT, RIGHT),
        (T_L, LEFT_SHOULDER),
        (T_R, RIGHT_SHOULDER),
        (T_L3, LEFT_STICK),
        (T_R3, RIGHT_STICK),
        // SDL maps the "menu" button to Back and the "view" button to Start.
        (T_VIEW, MENU),
        (T_MENU, VIEW),
        (T_STEAM, MENU),
        (T_QAM, VIEW),
    ];
    Some(Reading {
        buttons: bits
            .iter()
            .filter(|(host, _)| buttons & host != 0)
            .fold(0, |all, (_, bit)| all | bit),
        left_trigger: trigger(5),
        right_trigger: trigger(7),
        left_x: axis(9),
        // The report's Y grows upwards, as XInput's does.
        left_y: axis(11),
        right_x: axis(13),
        right_y: axis(15),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(id: u8, buttons: u32, trigger_l: i16, trigger_r: i16) -> Vec<u8> {
        let mut data = vec![0u8; 1 + 45];
        data[0] = id;
        data[1] = 7; // sequence number
        data[2..6].copy_from_slice(&buttons.to_le_bytes());
        data[6..8].copy_from_slice(&trigger_l.to_le_bytes());
        data[8..10].copy_from_slice(&trigger_r.to_le_bytes());
        data[10..12].copy_from_slice(&(-32767i16).to_le_bytes()); // left x
        data[12..14].copy_from_slice(&32767i16.to_le_bytes()); // left y
        data[14..16].copy_from_slice(&16384i16.to_le_bytes()); // right x
        data[16..18].copy_from_slice(&(-8192i16).to_le_bytes()); // right y
        data
    }

    #[test]
    fn face_buttons_and_triggers_map() {
        let reading = parse(&packet(STATE, T_A | T_B | T_L | T_R, 32767, 16384)).unwrap();
        assert_eq!(reading.buttons, A | B | LEFT_SHOULDER | RIGHT_SHOULDER);
        assert_eq!(reading.left_trigger, 1.0);
        assert!((reading.right_trigger - 0.5).abs() < 0.01);
    }

    #[test]
    fn sticks_are_signed_and_centered() {
        let reading = parse(&packet(STATE_BLE, 0, 0, 0)).unwrap();
        assert_eq!(reading.left_x, -1.0);
        assert_eq!(reading.left_y, 1.0);
        assert!((reading.right_x - 0.5).abs() < 0.01);
        assert!((reading.right_y + 0.25).abs() < 0.01);
    }

    #[test]
    fn menu_and_view_follow_sdl() {
        // SDL maps the controller's "menu" button to Back (VIEW here) and its
        // "view" button to Start (MENU here).
        assert_eq!(
            parse(&packet(STATE_TIMESTAMP, T_MENU, 0, 0))
                .unwrap()
                .buttons,
            VIEW
        );
        assert_eq!(parse(&packet(STATE, T_VIEW, 0, 0)).unwrap().buttons, MENU);
    }

    #[test]
    fn dpad_and_sticks_click() {
        let reading = parse(&packet(STATE, T_DPAD_UP | T_DPAD_LEFT | T_L3 | T_R3, 0, 0)).unwrap();
        assert_eq!(reading.buttons, UP | LEFT | LEFT_STICK | RIGHT_STICK);
    }

    #[test]
    fn other_reports_are_ignored() {
        assert!(parse(&[0x43, 0, 0]).is_none());
        assert!(parse(&[STATE]).is_none());
    }
}
