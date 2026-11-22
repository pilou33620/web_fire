//! Le serveur local du simulateur : sert l'interface (web/) et l'API, fait tourner la partie en temps réel.
//!
//! Lancement :  web_fire            (puis ouvrir http://127.0.0.1:8797)
//! Options   :  --port 8797  --lan (ouvert au réseau local, pour l'iPad)  --no-browser
//!              (et les options communes des web tools : --local, --sans-navigateur, --sans-pause, --sans-maj)
//! Avec --lan, un autre appareil saisit une fois le code d'appairage affiché dans la console (web_tools_commun).
//!
//! La simulation tourne dans un fil à part ; l'interface interroge /api/etat quelques fois par seconde.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::{json, Value};
use web_tools_commun::appairage::{self, Garde};
use web_tools_commun::production;
use web_tools_commun::serveur::{decoder_chemin, ErreurCorps, Reponse, Requete, Serveur};

pub const PORT: u16 = 8797;
const DEPOT: &str = "https://github.com/pilou33620/web_fire";
/// Une action de la page tient en quelques centaines d'octets.
const CORPS_MAX: usize = 1024 * 1024;
const JSON: &str = "application/json; charset=utf-8";

/// La racine de l'outil (web/) : le dossier courant s'il contient web/index.html, sinon le premier
/// dossier au-dessus de l'exécutable qui le contient.
pub fn racine() -> &'static Path {
    static R: OnceLock<PathBuf> = OnceLock::new();
    R.get_or_init(|| {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let outil = |d: &Path| d.join("web").join("index.html").is_file();
        if outil(&cwd) {
            return cwd;
        }
        let exe = std::env::current_exe().ok().and_then(|e| std::fs::canonicalize(e).ok());
        exe.and_then(|e| e.ancestors().find(|d| outil(d)).map(Path::to_path_buf)).unwrap_or(cwd)
    })
}

/// Numéro de partie (millisecondes modulo 10⁹, comme l'ancienne version) : la page voit qu'une nouvelle partie commence.
fn horodatage() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| (d.as_millis() % 1_000_000_000) as u64).unwrap_or(1)
}

fn gzip(octets: &[u8]) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::new(5));
    let _ = e.write_all(octets);
    e.finish().unwrap_or_default()
}

/// La partie, la vitesse (minutes de jeu par seconde) et la pause ; la carte est gardée prête, compressée.
pub struct Simulation {
    pub sim: feu::Sim,
    pub vitesse: f64,
    pub pause: bool,
    carte_json: Vec<u8>,
    carte_gz: Vec<u8>,
}

impl Default for Simulation {
    fn default() -> Self {
        Simulation::new()
    }
}

impl Simulation {
    pub fn new() -> Simulation {
        let mut s = Simulation { sim: feu::Sim::new(horodatage()), vitesse: 1.0, pause: false, carte_json: Vec::new(), carte_gz: Vec::new() };
        s.preparer_carte();
        s
    }

    fn preparer_carte(&mut self) {
        self.carte_json = self.sim.partie.export_carte().to_string().into_bytes();
        self.carte_gz = gzip(&self.carte_json);
    }

    pub fn action(&mut self, a: &Value) -> Result<Value, String> {
        match a.get("type").and_then(Value::as_str) {
            Some("vitesse") => {
                self.vitesse = a.get("valeur").map(feu::nombre).transpose()?.unwrap_or(1.0);
                self.pause = false;
                Ok(json!({"ok": true}))
            }
            Some("pause") => {
                self.pause = a.get("valeur").map_or(!self.pause, feu::vrai);
                Ok(json!({"ok": true}))
            }
            Some("nouvelle") => {
                let r = self.sim.action(a, horodatage())?;
                self.preparer_carte();
                Ok(r)
            }
            _ => self.sim.action(a, horodatage()),
        }
    }
}

fn verrou(s: &Mutex<Simulation>) -> MutexGuard<'_, Simulation> {
    s.lock().unwrap_or_else(|e| e.into_inner())
}

/// Le temps qui passe : `vitesse` minutes de jeu par seconde réelle.
fn boucle(sim: Arc<Mutex<Simulation>>) {
    let mut reste = 0.0;
    let mut dernier = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(50));
        let maintenant = Instant::now();
        let dt = maintenant.duration_since(dernier).as_secs_f64();
        dernier = maintenant;
        let (pause, vitesse) = {
            let s = verrou(&sim);
            (s.pause, s.vitesse)
        };
        if pause {
            continue;
        }
        reste += dt * vitesse;
        let mut n = 0;
        while reste >= 1.0 && n < 20 {
            reste -= 1.0;
            n += 1;
            let mut s = verrou(&sim);
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.sim.partie.pas())).is_err() {
                eprintln!("Erreur du moteur : la simulation est mise en pause.");
                s.pause = true;
                break;
            }
        }
        if reste > 5.0 {
            reste = 0.0; // la machine ne suit pas : on ralentit plutôt que d'accumuler du retard
        }
    }
}

/// Une réponse, compressée si le navigateur l'accepte et qu'elle en vaut la peine.
fn envoyer(code: u16, typ: &str, corps: Vec<u8>, deja: Option<&[u8]>, gz_ok: bool) -> Reponse {
    let gz = match (gz_ok, deja) {
        (false, _) => None,
        (true, Some(g)) => Some(g.to_vec()),
        (true, None) if corps.len() > 1400 && (typ.starts_with("application/json") || typ.starts_with("text")) => Some(gzip(&corps)),
        _ => None,
    };
    let rep = match gz {
        Some(g) => Reponse::new(code, typ, g).entete("Content-Encoding", "gzip"),
        None => Reponse::new(code, typ, corps),
    };
    rep.entete("Vary", "Accept-Encoding")
}

fn type_mime(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

/// Un fichier de web/ ; rien hors de ce dossier, ni fichier caché, ni extension inconnue.
fn fichier(chemin: &str, gz_ok: bool) -> Reponse {
    let introuvable = || Reponse::texte(404, "introuvable");
    let rel = if chemin == "/" { "index.html".to_string() } else { decoder_chemin(chemin.trim_start_matches('/')) };
    let p = Path::new(&rel);
    let sur = !rel.is_empty() && p.components().all(|c| matches!(c, Component::Normal(s) if !s.to_string_lossy().starts_with('.')));
    let Some(typ) = p.extension().and_then(|x| x.to_str()).and_then(type_mime).filter(|_| sur) else { return introuvable() };
    let web = racine().join("web");
    // pas de lien symbolique qui sorte de web/
    let (Ok(f), Ok(w)) = (std::fs::canonicalize(web.join(p)), std::fs::canonicalize(&web)) else { return introuvable() };
    if !f.starts_with(&w) || !f.is_file() {
        return introuvable();
    }
    match std::fs::read(&f) {
        Ok(o) => {
            let mut rep = envoyer(200, typ, o, None, gz_ok);
            if typ != "text/html; charset=utf-8" {
                rep.entetes.retain(|(k, _)| !k.eq_ignore_ascii_case("Cache-Control"));
                rep = rep.entete("Cache-Control", "max-age=60");
            }
            rep
        }
        Err(_) => introuvable(),
    }
}

fn garde() -> &'static Garde {
    GARDE.get().expect("garde non initialisée")
}
static GARDE: OnceLock<Garde> = OnceLock::new();

fn servir(r: &mut Requete, sim: &Arc<Mutex<Simulation>>) -> Reponse {
    // Host, Internet, TLS, jeton : avant tout traitement de la requête
    if let Some(rep) = garde().filtrer(r) {
        return rep;
    }
    let gz_ok = r.entete("Accept-Encoding").unwrap_or("").contains("gzip");
    let chemin = r.chemin().to_string();
    if r.lecture() {
        return match chemin.as_str() {
            "/api/carte" => {
                let s = verrou(sim);
                envoyer(200, JSON, s.carte_json.clone(), Some(s.carte_gz.as_slice()), gz_ok)
            }
            "/api/etat" => {
                let depuis = r.param("depuis").and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(0);
                let e = {
                    let s = verrou(sim);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.sim.etat(depuis, s.vitesse, s.pause)))
                };
                match e {
                    Ok(v) => envoyer(200, JSON, v.to_string().into_bytes(), None, gz_ok),
                    Err(_) => Reponse::json(500, &json!({"ok": false, "erreur": "erreur interne du moteur"})),
                }
            }
            "/api/infos" => envoyer(200, JSON, feu::infos().to_string().into_bytes(), None, gz_ok),
            _ => fichier(&chemin, gz_ok),
        };
    }
    if r.methode != "POST" {
        return Reponse::texte(405, "méthode refusée");
    }
    // une page d'un autre site ne peut pas envoyer de JSON sans pré-vol CORS, que ce serveur ne valide jamais
    if !r.entete("Content-Type").unwrap_or("").trim().to_ascii_lowercase().starts_with("application/json") {
        return Reponse::json(403, &json!({"detail": "Requête refusée"}));
    }
    let corps = match r.lire_corps(CORPS_MAX) {
        Ok(c) => c,
        Err(ErreurCorps::TropGrand) => return Reponse::json(413, &json!({"ok": false, "erreur": "Requête trop grande"})),
        Err(_) => return Reponse::json(400, &json!({"ok": false, "erreur": "Requête illisible"})),
    };
    let a: Value = match serde_json::from_slice(if corps.is_empty() { b"{}" } else { &corps }) {
        Ok(v @ Value::Object(_)) => v,
        _ => return Reponse::json(400, &json!({"ok": false, "erreur": "JSON invalide"})),
    };
    if chemin != "/api/action" {
        return Reponse::json(404, &json!({"ok": false, "erreur": "inconnu"}));
    }
    let res = {
        let mut s = verrou(sim);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.action(&a)))
    };
    match res {
        Ok(Ok(v)) => envoyer(200, JSON, v.to_string().into_bytes(), None, gz_ok),
        Ok(Err(e)) => Reponse::json(500, &json!({"ok": false, "erreur": e})),
        Err(_) => Reponse::json(500, &json!({"ok": false, "erreur": "erreur interne du moteur"})),
    }
}

/// En-têtes de sécurité sur toutes les réponses (X-Frame-Options vient déjà de la caisse commune).
fn avec_entetes(rep: Reponse) -> Reponse {
    rep.entete("Content-Security-Policy", "frame-ancestors 'self'").entete("X-Content-Type-Options", "nosniff")
}

// ---------------------------------------------------------------------- lancement

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub port: u16,
    pub lan: bool,
    pub local: bool,
    pub no_browser: bool,
}

pub const USAGE: &str = "usage: web_fire [--port 8797] [--lan | --local] [--no-browser | --sans-navigateur] [--sans-pause] [--sans-maj]";

pub fn lire_args(argv: &[String]) -> Result<Args, String> {
    let mut a = Args { port: PORT, ..Default::default() };
    let mut i = 0;
    while i < argv.len() {
        let x = argv[i].as_str();
        let (cle, val) = match x.split_once('=') {
            Some((k, v)) => (k, Some(v.to_string())),
            None => (x, None),
        };
        match cle {
            "--port" => {
                let v = match val {
                    Some(v) => v,
                    None => {
                        i += 1;
                        argv.get(i).cloned().ok_or("--port attend un nombre")?
                    }
                };
                a.port = v.parse().map_err(|_| format!("port invalide : {v}"))?;
            }
            "--lan" if val.is_none() => a.lan = true,
            "--local" if val.is_none() => a.local = true,
            "--no-browser" | "--sans-navigateur" if val.is_none() => a.no_browser = true,
            // acceptées pour le launcher : rien à attendre à l'arrêt, pas de mise à jour propre à l'outil
            "--sans-pause" | "--sans-maj" | "--no-update" if val.is_none() => {}
            _ => return Err(format!("option inconnue : {x}")),
        }
        i += 1;
    }
    Ok(a)
}

/// Copie de production (archive web_fire-prod, sans git) : mise à jour depuis la publication
/// « binaires » (§4.1 bis des règles). Sauf --sans-maj, et sauf à la relance qui suit une mise à jour.
fn maj_production(argv: &[String]) {
    let sans_maj = argv.iter().any(|x| x == "--sans-maj" || x == "--no-update");
    if sans_maj || std::env::var_os("WEB_FIRE_DEJA_MAJ").is_some() || !production::est_copie_prod(racine()) {
        return;
    }
    match production::mettre_a_jour(DEPOT, "web_fire", racine(), &|t| println!("{t}")) {
        // demarrer_WEB_FIRE relance la nouvelle version ; lancé sans lui, on continue avec celle-ci
        Ok(true) if std::env::var_os("WEB_FIRE_RELANCEUR").is_some() => std::process::exit(production::CODE_MIS_A_JOUR),
        Ok(true) => println!("Mis à jour : relancez le simulateur pour en profiter."),
        Ok(false) => {}
        Err(e) => println!("[X] {e}"), // l'installation actuelle est intacte : on démarre
    }
}

fn ouvrir_navigateur(url: &str) {
    let r = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", url]).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
    };
    if r.is_err() {
        println!("Ouvrez {url} dans votre navigateur.");
    }
}

pub fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match lire_args(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{USAGE}\nweb_fire: {e}");
            std::process::exit(2);
        }
    };
    let _ = std::env::set_current_dir(racine());
    maj_production(&argv);
    println!("Préparation de la carte…");
    let _ = std::io::stdout().flush();
    let sim = Arc::new(Mutex::new(Simulation::new()));
    let reseau = args.lan && !args.local;
    let srv = match Serveur::ouvrir(args.port, reseau, 10) {
        Ok(s) => s,
        Err(e) => {
            println!("Impossible d'ouvrir le port {} ({e}). Le simulateur tourne peut-être déjà.", args.port);
            std::process::exit(1);
        }
    };
    let port = srv.port();
    if port != args.port {
        println!("(le port {} était occupé)", args.port);
    }
    let _ = GARDE.set(Garde::new("Web Fire", reseau, None, None));
    garde().brancher(&srv); // avec --lan : HTTPS sur le même port
    let url = format!("http://127.0.0.1:{port}");
    println!("Simulateur prêt : {url}");
    if reseau {
        for ip in appairage::adresses_poste() {
            println!("Sur le réseau local : {}", garde().adresse(&ip, port));
        }
        println!("  l'autre appareil demande une fois le code d'appairage ci-dessous");
        garde().annoncer();
    }
    let _ = std::io::stdout().flush();
    {
        let s = sim.clone();
        std::thread::spawn(move || boucle(s));
    }
    let ecran = cfg!(target_os = "windows") || cfg!(target_os = "macos") || std::env::var_os("DISPLAY").is_some();
    if !args.no_browser && ecran && std::env::var_os("TERMUX_VERSION").is_none() {
        let u = url.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(800));
            ouvrir_navigateur(&u);
        });
    }
    srv.servir(Arc::new(move |r: &mut Requete| avec_entetes(servir(r, &sim))));
}

#[cfg(test)]
mod essais {
    use super::*;
    use std::io::Read;
    use std::net::TcpStream;

    fn a(l: &[&str]) -> Vec<String> {
        l.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn ligne_de_commande() {
        // la commande du catalogue de web_launcher, puis les options communes des web tools
        let x = lire_args(&a(&["--lan", "--port", "8797", "--no-browser"])).unwrap();
        assert_eq!((x.port, x.lan, x.no_browser), (8797, true, true));
        let x = lire_args(&a(&["--port=9000", "--local", "--sans-navigateur", "--sans-pause", "--sans-maj", "--no-update"])).unwrap();
        assert_eq!((x.port, x.local, x.no_browser), (9000, true, true));
        assert_eq!(lire_args(&[]).unwrap().port, PORT);
        assert!(lire_args(&a(&["--port"])).is_err());
        assert!(lire_args(&a(&["--port", "abc"])).is_err());
        assert!(lire_args(&a(&["--inconnu"])).is_err());
        assert!(lire_args(&a(&["--lan=1"])).is_err());
    }

    fn serveur() -> u16 {
        static PORT_ESSAI: OnceLock<u16> = OnceLock::new();
        *PORT_ESSAI.get_or_init(|| {
            let _ = GARDE.set(Garde::new("Web Fire", false, None, None));
            let srv = Serveur::ouvrir(0, false, 1).expect("port");
            let sim = Arc::new(Mutex::new(Simulation::new()));
            let port = srv.port();
            srv.servir_en_fond(Arc::new(move |r: &mut Requete| avec_entetes(servir(r, &sim))));
            port
        })
    }

    /// (code, en-têtes et corps) d'une requête brute.
    fn requete(brut: &str) -> (u16, String) {
        let port = serveur();
        let mut t = TcpStream::connect(("127.0.0.1", port)).unwrap();
        t.write_all(brut.replace("{port}", &port.to_string()).as_bytes()).unwrap();
        let mut rep = Vec::new();
        t.read_to_end(&mut rep).unwrap();
        let rep = String::from_utf8_lossy(&rep).to_string();
        let code = rep.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        (code, rep)
    }

    fn get(chemin: &str) -> (u16, String) {
        requete(&format!("GET {chemin} HTTP/1.0\r\nHost: 127.0.0.1:{{port}}\r\n\r\n"))
    }

    fn post(chemin: &str, entetes: &str, corps: &str) -> (u16, String) {
        requete(&format!("POST {chemin} HTTP/1.0\r\nHost: 127.0.0.1:{{port}}\r\n{entetes}Content-Length: {}\r\n\r\n{corps}", corps.len()))
    }

    #[test]
    fn fichiers_statiques() {
        let (c, r) = get("/");
        assert_eq!(c, 200);
        assert!(r.contains("text/html") && r.contains("nosniff") && r.contains("frame-ancestors"));
        let (c, r) = get("/js/liaison.js");
        assert_eq!(c, 200);
        assert!(r.contains("text/javascript"));
        for chemin in ["/../Cargo.toml", "/Cargo.toml", "/.git/config", "/.env", "/%2e%2e/Cargo.toml", "/js/../../Cargo.toml", "/vendor/three/LICENSE"] {
            assert_eq!(get(chemin).0, 404, "{chemin}");
        }
        assert_eq!(type_mime("wasm"), Some("application/wasm"));
    }

    #[test]
    fn api() {
        let (c, r) = get("/api/infos");
        assert_eq!(c, 200);
        assert!(r.contains("\"scenarios\""));
        let (c, r) = get("/api/carte");
        assert_eq!(c, 200);
        assert!(r.contains("\"fuel\""));
        let (c, r) = get("/api/etat?depuis=0");
        assert_eq!(c, 200);
        assert!(r.contains("\"meteo\"") && r.contains("\"vitesse\""));
        let json = "Content-Type: application/json\r\n";
        let (c, r) = post("/api/action", json, r#"{"type":"pause","valeur":true}"#);
        assert_eq!(c, 200);
        assert!(r.contains("\"ok\":true"));
        assert_eq!(post("/api/action", json, r#"{"type":"allumer"}"#).0, 500);
        assert_eq!(post("/api/autre", json, "{}").0, 404);
    }

    #[test]
    fn securite() {
        assert_eq!(requete("GET / HTTP/1.0\r\nHost: piege.example\r\n\r\n").0, 403);
        let json = "Content-Type: application/json\r\n";
        // sans JSON déclaré : refusé (une page d'un autre site pourrait l'envoyer)
        assert_eq!(post("/api/action", "Content-Type: text/plain\r\n", r#"{"type":"pause"}"#).0, 403);
        assert_eq!(post("/api/action", "", r#"{"type":"pause"}"#).0, 403);
        assert_eq!(post("/api/action", json, "{pas du json").0, 400);
        assert_eq!(post("/api/action", json, "[1, 2]").0, 400);
        let (c, _) = requete(&format!(
            "POST /api/action HTTP/1.0\r\nHost: 127.0.0.1:{{port}}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            CORPS_MAX + 1
        ));
        assert_eq!(c, 413);
        assert_eq!(requete("DELETE /api/action HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n").0, 405);
    }

    #[test]
    fn compression() {
        let (c, r) = requete("GET /api/carte HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nAccept-Encoding: gzip\r\n\r\n");
        assert_eq!(c, 200);
        assert!(r.contains("Content-Encoding: gzip"));
    }
}
