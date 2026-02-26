use std::sync::{OnceLock, RwLock, mpsc};

use crate::lighting::LightingData;
mod config;
mod lighting;

static LOG: OnceLock<extern "C" fn(c: *const u8)> = OnceLock::new();
static DATA: OnceLock<RwLock<LightingData>> = OnceLock::new();
static SENDER: OnceLock<mpsc::Sender<Option<LightingData>>> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "C" fn GetName() -> *const u8 {
    c"Bindable Lighting".as_ptr() as *const u8
}

#[unsafe(no_mangle)]
pub extern "C" fn SetButtons(bitfield: u32) {
    let mut data = DATA.get().unwrap().write().unwrap();
    for i in 0..7 {
        data.buttons[i] = (bitfield & (1 << i)) != 0;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn SetLights(left: u8, pos: u32, r: u8, g: u8, b: u8) {
    let mut data = DATA.get().unwrap().write().unwrap();
    let side = if left == 1 { 0 } else { 1 };
    let target = match pos {
        0 => &mut data.bottom[side],
        1 => &mut data.middle[side],
        2 => &mut data.top[side],
        _ => unreachable!(),
    };

    target.r = r as f32 / 255.0;
    target.g = g as f32 / 255.0;
    target.b = b as f32 / 255.0;
}

#[unsafe(no_mangle)]
pub extern "C" fn Tick(_delta_time: f32) {
    let data = *DATA.get().unwrap().read().unwrap();

    SENDER.get().unwrap().send(Some(data)).unwrap();
}
#[unsafe(no_mangle)]
pub extern "C" fn Close() {
    SENDER.get().unwrap().send(None).unwrap();
}

#[unsafe(no_mangle)]
pub extern "C" fn Init(log: extern "C" fn(c: *const u8)) -> i32 {
    std::panic::set_hook(Box::new(|e| {
        if let Some(log) = LOG.get() {
            log(e.to_string().as_ptr());
        }
    }));

    if LOG.set(log).is_err() {
        return 1;
    }

    if DATA.set(LightingData::default().into()).is_err() {
        return 1;
    }

    let (tx, rx) = mpsc::channel();

    if SENDER.set(tx).is_err() {
        return 1;
    }

    std::thread::spawn(move || lighting::lighting_worker(rx));

    0
}
