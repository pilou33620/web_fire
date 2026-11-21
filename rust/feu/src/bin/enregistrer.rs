//! Enregistre une journée jouée par le vrai moteur, pour la démo rejouée quand le moteur ne peut pas tourner dans la page.
//!
//!     cargo run --release -p feu --bin enregistrer -- --sortie web/demo/rejeu.json --graine 7 --scenario canicule --minutes 600 --pas 3

use serde_json::{json, Value};

fn main() {
    let mut sortie = "web/demo/rejeu.json".to_string();
    let (mut graine, mut theme, mut scenario, mut minutes, mut pas) = (7i64, "gironde".to_string(), "canicule".to_string(), 600usize, 3usize);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned().unwrap_or_default();
        let ok = match args[i].as_str() {
            "--sortie" => {
                sortie = val;
                true
            }
            "--graine" => val.parse().map(|v| graine = v).is_ok(),
            "--theme" => {
                theme = val;
                true
            }
            "--scenario" => {
                scenario = val;
                true
            }
            "--minutes" => val.parse().map(|v| minutes = v).is_ok(),
            "--pas" => val.parse().map(|v: usize| pas = v.max(1)).is_ok(),
            _ => false,
        };
        if !ok {
            eprintln!("usage: enregistrer [--sortie F] [--graine N] [--theme T] [--scenario S] [--minutes N] [--pas N]");
            std::process::exit(2);
        }
        i += 2;
    }
    let mut p = feu::Partie::new(graine, &theme, &scenario, &feu::partie::Config::new(), "equilibree", true);
    let carte = p.export_carte(); // la carte du début (les bulldozers la modifient ensuite)
    let (mut images, mut journal, mut evts, mut seq) = (Vec::new(), Vec::new(), Vec::new(), 0);
    for m in 0..=minutes {
        if m % pas == 0 {
            let mut e = p.export_etat(seq);
            let o = e.as_object_mut().expect("état");
            journal.extend(o.remove("journal").and_then(|v| v.as_array().cloned()).unwrap_or_default());
            evts.extend(o.remove("evts").and_then(|v| v.as_array().cloned()).unwrap_or_default().into_iter().filter(|v| v["type"] == "largage"));
            seq = o["seq"].as_i64().unwrap_or(seq);
            // on allège : le front sert au calque de prévision
            for f in o["feux"].as_array_mut().into_iter().flatten() {
                let front: Vec<Value> = f["front"].as_array().map(|a| a.iter().take(30).cloned().collect()).unwrap_or_default();
                f["front"] = json!(front);
                f["flancs"] = json!([]);
            }
            for u in o["moyens"].as_array_mut().into_iter().flatten() {
                if let Some(u) = u.as_object_mut() {
                    u.remove("v");
                }
            }
            images.push(e);
        }
        p.pas();
    }
    let mut defaut = serde_json::Map::new();
    for &(k, v) in feu::MOYENS_DEFAUT {
        defaut.insert(k.into(), json!(v));
    }
    let n = images.len();
    let d = json!({"pas": pas, "carte": carte, "infos": feu::infos(), "images": images, "journal": journal, "evts": evts,
                   "reglages": {"graine": graine, "theme": theme, "scenario": scenario, "moyens": defaut, "doctrine": "equilibree"}});
    if let Some(dossier) = std::path::Path::new(&sortie).parent() {
        let _ = std::fs::create_dir_all(dossier);
    }
    let texte = d.to_string();
    if let Err(e) = std::fs::write(&sortie, &texte) {
        eprintln!("Écriture impossible de {sortie} : {e}");
        std::process::exit(1);
    }
    let (_, st) = p.bilan();
    println!("Démo enregistrée : {sortie} ({n} images, {} Ko, {:.0} ha brûlés)", texte.len() / 1024, st["ha"].as_f64().unwrap_or(0.0));
}
