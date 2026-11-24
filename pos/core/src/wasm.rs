use wasm_bindgen::prelude::*;

use crate::store::Store;

#[wasm_bindgen]
pub struct PosSimulator {
    store: Store,
}

#[wasm_bindgen]
impl PosSimulator {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, registers: u8, bells: u8) -> PosSimulator {
        console_error_panic_hook::set_once();
        PosSimulator {
            store: Store::new(u64::from(seed), registers, bells),
        }
    }

    pub fn restore(saved_json: &str) -> Result<PosSimulator, JsValue> {
        console_error_panic_hook::set_once();
        Store::load_json(saved_json)
            .map(|store| PosSimulator { store })
            .map_err(|e| JsValue::from_str(&e))
    }

    /// レジを一台足す。足せたら true
    pub fn add_register(&mut self) -> bool {
        self.store.add_register()
    }

    /// Bell の端末を一台足す（"fixed" か "handheld"）。足せたら true
    pub fn add_bell(&mut self, kind: &str) -> bool {
        let kind = match kind {
            "fixed" => crate::store::BellKind::Fixed,
            "handheld" => crate::store::BellKind::Handheld,
            _ => return false,
        };
        self.store.add_bell(kind)
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
