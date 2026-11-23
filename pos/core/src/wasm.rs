use wasm_bindgen::prelude::*;

use crate::store::rush::{Rush, RushParams};
use crate::store::Store;

#[wasm_bindgen]
pub struct PosSimulator {
    store: Store,
    rush: Option<Rush>,
}

#[wasm_bindgen]
impl PosSimulator {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32, registers: u8, bells: u8) -> PosSimulator {
        console_error_panic_hook::set_once();
        PosSimulator {
            store: Store::new(u64::from(seed), registers, bells),
            rush: None,
        }
    }

    pub fn restore(saved_json: &str) -> Result<PosSimulator, JsValue> {
        console_error_panic_hook::set_once();
        Store::load_json(saved_json)
            .map(|store| PosSimulator { store, rush: None })
            .map_err(|e| JsValue::from_str(&e))
    }

    /// レジを一台足す。足せたら true
    pub fn add_register(&mut self) -> bool {
        self.store.add_register()
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

    /// 混雑シミュレーションを始める。いまの店（取引のないレジ）でお客様とスタッフを動かす
    pub fn rush_start(&mut self, params_json: &str) -> Result<(), JsValue> {
        let params: RushParams =
            serde_json::from_str(params_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.rush = Some(Rush::new(params, &mut self.store));
        Ok(())
    }

    /// 混雑シミュレーションの時計を進める（店の時計も一緒に進む）
    pub fn rush_advance(&mut self, ms: f64) {
        if let Some(r) = self.rush.as_mut() {
            r.advance(&mut self.store, ms.max(0.0) as u64);
        }
    }

    /// 混雑シミュレーションの様子（JSON）。動いていなければ "null"
    pub fn rush_view(&self) -> String {
        match &self.rush {
            Some(r) => serde_json::to_string(&r.view(&self.store)).unwrap_or_else(|_| "null".into()),
            None => "null".into(),
        }
    }

    pub fn rush_stop(&mut self) {
        self.rush = None;
    }

    /// 画面とは別の店で、最後まで一気に流した結果（JSON）
    pub fn rush_run(params_json: &str) -> Result<String, JsValue> {
        let params: RushParams =
            serde_json::from_str(params_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_json::to_string(&Rush::run(params)).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
