use wasm_bindgen::prelude::*;

use crate::store::Store;

#[wasm_bindgen]
pub struct PosSimulator {
    store: Store,
}

#[wasm_bindgen]
impl PosSimulator {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, registers: u8, mats: u8) -> PosSimulator {
        console_error_panic_hook::set_once();
        PosSimulator {
            store: Store::new(u64::from(seed), registers, mats),
        }
    }

    pub fn restore(saved_json: &str) -> Result<PosSimulator, JsValue> {
        console_error_panic_hook::set_once();
        Store::load_json(saved_json)
            .map(|store| PosSimulator { store })
            .map_err(|e| JsValue::from_str(&e))
    }

    pub fn save(&self) -> String {
        self.store.save_json()
    }

    pub fn dispatch(&mut self, command_json: &str) -> String {
        crate::dispatch_json(&mut self.store, command_json)
    }

    pub fn tick(&mut self, now_ms: f64) {
        self.store.tick(now_ms.max(0.0) as u64);
    }

    pub fn snapshot(&self) -> String {
        crate::snapshot_json(&self.store)
    }
}
