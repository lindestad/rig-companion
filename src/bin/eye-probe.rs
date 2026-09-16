//! Isolated, read-only probe of the installed Pimax calibration backend.
//! ABI verified against EyeTrackingGuide 5.9.0.2 interop declarations.
use anyhow::{Result, ensure};
use libloading::Library;
use serde_json::json;
use std::ffi::{CStr, CString, c_char, c_void};

type Handle = *mut c_void;
type UrlCallback = unsafe extern "C" fn(*const c_char, Handle);
type DataCallback = unsafe extern "C" fn(*const c_void, usize, Handle);

unsafe extern "C" fn receive_url(url: *const c_char, context: Handle) {
    if !url.is_null() {
        // SAFETY: enumeration invokes callbacks synchronously with the caller's Vec.
        unsafe {
            (&mut *context.cast::<Vec<CString>>()).push(CStr::from_ptr(url).to_owned());
        }
    }
}
unsafe extern "C" fn receive_size(_: *const c_void, size: usize, context: Handle) {
    // Count only. No calibration bytes or gaze samples are copied or stored.
    unsafe {
        *context.cast::<usize>() = size;
    }
}

fn main() {
    match probe() {
        Ok(value) => println!("{value}"),
        Err(e) => {
            println!("{}", json!({"ok":false,"error":format!("{e:#}")}));
            std::process::exit(1);
        }
    }
}

fn probe() -> Result<serde_json::Value> {
    let path = r"C:\Program Files\Pimax\EyeTrackingGuide\EyeTrackingGuide_Data\Plugins\x86_64\tobii_stream_engine.dll";
    // SAFETY: absolute installed vendor DLL, cdecl ABI, bounded output layouts; version
    // checked before device calls. The library outlives every symbol and handle.
    unsafe {
        let lib = Library::new(path)?;
        let version =
            lib.get::<unsafe extern "C" fn(*mut [i32; 4]) -> i32>(b"tobii_get_api_version\0")?;
        let mut v = [0; 4];
        ensure!(
            version(&mut v) == 0 && v == [5, 9, 0, 2],
            "Unsupported Tobii ABI version: {v:?}"
        );
        let create = lib.get::<unsafe extern "C" fn(*mut Handle, Handle, Handle) -> i32>(
            b"tobii_api_create\0",
        )?;
        let destroy = lib.get::<unsafe extern "C" fn(Handle) -> i32>(b"tobii_api_destroy\0")?;
        let enumerate = lib.get::<unsafe extern "C" fn(Handle, UrlCallback, Handle) -> i32>(
            b"tobii_enumerate_local_device_urls\0",
        )?;
        let device_create =
            lib.get::<unsafe extern "C" fn(Handle, *const c_char, i32, *mut Handle) -> i32>(
                b"tobii_device_create\0",
            )?;
        let device_destroy =
            lib.get::<unsafe extern "C" fn(Handle) -> i32>(b"tobii_device_destroy\0")?;
        let info =
            lib.get::<unsafe extern "C" fn(Handle, *mut u8) -> i32>(b"tobii_get_device_info\0")?;
        let capability = lib.get::<unsafe extern "C" fn(Handle, i32, *mut i32) -> i32>(
            b"tobii_capability_supported\0",
        )?;
        let retrieve = lib.get::<unsafe extern "C" fn(Handle, DataCallback, Handle) -> i32>(
            b"tobii_calibration_retrieve\0",
        )?;
        let error_message =
            lib.get::<unsafe extern "C" fn(i32) -> *const c_char>(b"tobii_error_message\0")?;
        let describe = |code| {
            let ptr = error_message(code);
            if ptr.is_null() {
                format!("error {code}")
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };
        let mut api = std::ptr::null_mut();
        let code = create(&mut api, std::ptr::null_mut(), std::ptr::null_mut());
        ensure!(code == 0 && !api.is_null(), "Tobii API: {}", describe(code));
        let result = (|| -> Result<_> {
            let mut urls: Vec<CString> = Vec::new();
            let code = enumerate(api, receive_url, (&mut urls as *mut Vec<CString>).cast());
            ensure!(code == 0, "Enumeration: {}", describe(code));
            let mut devices = Vec::new();
            for url in urls {
                let mut device = std::ptr::null_mut();
                // Field-of-use 1 = no storage/transfer. No vendor license injection.
                let code = device_create(api, url.as_ptr(), 1, &mut device);
                if code != 0 || device.is_null() {
                    devices.push(json!({"connected":false,"error":describe(code)}));
                    continue;
                }
                let mut bytes = [0u8; 2304];
                let code = info(device, bytes.as_mut_ptr());
                let details = if code != 0 {
                    json!({"connected":true,"error":describe(code)})
                } else {
                    let field = |start, end| {
                        String::from_utf8_lossy(&bytes[start..end])
                            .trim_end_matches('\0')
                            .to_string()
                    };
                    let model = field(256, 512);
                    let generation = field(512, 768);
                    let mut supports = 0;
                    let cap_code = capability(device, 2, &mut supports); // CALIBRATION_3D
                    let mut size = 0usize;
                    // Query backup availability only on the XR5 wearable identified by the vendor.
                    let backup = if generation.to_ascii_uppercase().contains("XR5") {
                        let code = retrieve(device, receive_size, (&mut size as *mut usize).cast());
                        json!({"result":describe(code),"code":code,"bytes_available":size})
                    } else {
                        json!({"result":"Not queried: device is not identified as XR5"})
                    };
                    json!({"connected":true,"model":model,"generation":generation,
                        "calibration_3d":cap_code == 0 && supports != 0,"capability_result":describe(cap_code),"backup":backup})
                };
                let cleanup = device_destroy(device);
                ensure!(cleanup == 0, "Device cleanup: {}", describe(cleanup));
                devices.push(details);
            }
            Ok(json!({"ok":true,"version":v,"devices":devices,"writes_performed":false}))
        })();
        let cleanup = destroy(api);
        ensure!(cleanup == 0, "API cleanup: {}", describe(cleanup));
        result
    }
}
