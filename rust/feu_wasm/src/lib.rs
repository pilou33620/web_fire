//! Le moteur dans le navigateur (WebAssembly, `wasm-bindgen --target web`) : la même interface que l'ancienne
//! liaison Pyodide. Les fonctions rendent du JSON en texte, que web/js/liaison.js relit par JSON.parse ;
//! la vitesse et la pause restent dans la page.

use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Sim {
    sim: feu::Sim,
    /// numéro de partie : il change à chaque nouvelle partie (la page recharge alors la carte)
    n: u64,
}

#[wasm_bindgen]
impl Sim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Sim {
        Sim { sim: feu::Sim::new(1), n: 1 }
    }

    pub fn carte(&self) -> String {
        self.sim.partie.export_carte().to_string()
    }

    pub fn infos(&self) -> String {
        feu::infos().to_string()
    }

    pub fn etat(&self, depuis: f64, vitesse: f64, pause: bool) -> String {
        self.sim.etat(depuis as i64, vitesse, pause).to_string()
    }

    pub fn pas(&mut self, n: u32) {
        for _ in 0..n {
            self.sim.partie.pas();
        }
    }

    pub fn action(&mut self, txt: &str) -> String {
        let a: Value = match serde_json::from_str(txt) {
            Ok(v @ Value::Object(_)) => v,
            _ => return json!({"ok": false, "erreur": "JSON invalide"}).to_string(),
        };
        self.n += 1;
        match self.sim.action(&a, self.n) {
            Ok(v) => v.to_string(),
            Err(e) => json!({"ok": false, "erreur": e}).to_string(),
        }
    }
}

impl Default for Sim {
    fn default() -> Self {
        Sim::new()
    }
}

#[cfg(test)]
mod essais {
    use super::Sim;

    #[test]
    fn meme_interface_que_pyodide() {
        let mut s = Sim::new();
        let infos: serde_json::Value = serde_json::from_str(&s.infos()).unwrap();
        assert!(infos["vitesses"].is_array());
        let e: serde_json::Value = serde_json::from_str(&s.etat(0.0, 2.0, true)).unwrap();
        assert_eq!(e["vitesse"], 2.0);
        assert_eq!(e["pause"], true);
        s.pas(3);
        let r: serde_json::Value = serde_json::from_str(&s.action(r#"{"type":"nouvelle","reglages":{"graine":9}}"#)).unwrap();
        assert_eq!(r["ok"], true);
        assert_ne!(r["partie"], e["partie"]);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&s.action("[]")).unwrap()["ok"], false);
        assert!(s.carte().contains("\"fuel\""));
    }
}
