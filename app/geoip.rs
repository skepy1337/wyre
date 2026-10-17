use anyhow::{Context, Result, anyhow, bail, ensure};
use maxminddb::{Mmap, PathElement, Reader, path};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

const DAY: Duration = Duration::from_hours(24);
const MONTH: Duration = Duration::from_hours(30 * 24);

const RELEASES: &str =
    "https://github.com/sapics/ip-location-db/releases/download/latest";

type Download = (&'static str, &'static str, &'static str);

const FREE_DATABASES: [Download; 2] = [
    ("country", "country-free", "user-country.mmdb"),
    ("network", "asn-free", "origin-asn.mmdb"),
];

const DBIP_DATABASES: [Download; 4] = [
    ("country", "country-dbip", "dbip-country.mmdb"),
    ("network", "asn-dbip", "dbip-asn.mmdb"),
    ("city (IPv4)", "city-dbip-v4", "dbip-city-ipv4.mmdb"),
    ("city (IPv6)", "city-dbip-v6", "dbip-city-ipv6.mmdb"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeoMode {
    Off,
    // open databases from
    // https://github.com/sapics/ip-location-db
    Free,
    DbIp,
    Imported,
}

impl GeoMode {
    const ALL: [GeoMode; 4] =
        [GeoMode::Off, GeoMode::Free, GeoMode::DbIp, GeoMode::Imported];

    pub fn as_str(&self) -> &'static str {
        match self {
            GeoMode::Off => "Off",
            GeoMode::Free => "Free",
            GeoMode::DbIp => "DbIp",
            GeoMode::Imported => "Imported",
        }
    }

    pub fn parse(name: &str) -> Option<GeoMode> {
        GeoMode::ALL.into_iter().find(|mode| mode.as_str() == name)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Geo {
    pub country: Option<String>,
    pub org: Option<String>,
    pub city: Option<String>,
    // for us states
    pub state: Option<String>,
}

#[derive(Clone, Default, Serialize)]
pub struct Imported {
    pub path: String,
    pub loaded: bool,
    // city, country, etc
    pub kind: String,
    pub has: String,
    pub error: String,
}

impl Imported {
    fn new(path: String, database: &Result<Database>) -> Imported {
        match database {
            Ok(database) => Imported {
                path,
                loaded: true,
                kind: database.kind(),
                has: database.contents(),
                error: String::new(),
            },
            Err(e) => Imported { path, error: format!("{e:#}"), ..Imported::default() },
        }
    }
}

pub struct GeoStatus {
    pub status: String,
    pub imported: Vec<Imported>,
}

pub struct GeoDb {
    cache_dir: PathBuf,
    import_dir: PathBuf,
    settings: Mutex<Settings>,
    databases: RwLock<Vec<Database>>,
    cache: Mutex<HashMap<IpAddr, Option<Geo>>>,
    loading: Mutex<()>,
    pages_touched: AtomicBool,
}

struct Settings {
    mode: GeoMode,
    imported: Vec<PathBuf>,
    status: String,
    imported_status: Vec<Imported>,
}

impl GeoDb {
    pub fn new() -> Arc<GeoDb> {
        let cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".cache"));
        let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".local/share"));
        Arc::new(GeoDb {
            cache_dir: cache_dir.join("wyre/geoip"),
            import_dir: data_dir.join("wyre/imported"),
            settings: Mutex::new(Settings {
                mode: GeoMode::Off,
                imported: Vec::new(),
                status: "Disabled".to_string(),
                imported_status: Vec::new(),
            }),
            databases: RwLock::new(Vec::new()),
            cache: Mutex::new(HashMap::new()),
            loading: Mutex::new(()),
            pages_touched: AtomicBool::new(false),
        })
    }

    pub fn status(&self) -> GeoStatus {
        let settings = self.settings.lock().unwrap();
        let imported = settings
            .imported
            .iter()
            .map(|path| {
                let path = path.display().to_string();
                let loaded =
                    settings.imported_status.iter().find(|status| status.path == path);
                loaded.cloned().unwrap_or(Imported { path, ..Imported::default() })
            })
            .collect();
        GeoStatus { status: settings.status.clone(), imported }
    }

    pub fn set_mode(self: &Arc<Self>, mode: GeoMode) {
        let mut settings = self.settings.lock().unwrap();
        if settings.mode != mode {
            settings.mode = mode;
            settings.status =
                if mode == GeoMode::Off { "Disabled" } else { "Checking…" }.to_string();
            self.databases.write().unwrap().clear();
            self.cache.lock().unwrap().clear();
        }
        drop(settings);
        if mode != GeoMode::Off {
            self.refresh();
        }
    }

    pub fn set_imported(self: &Arc<Self>, imported: Vec<PathBuf>) {
        let mut settings = self.settings.lock().unwrap();
        for removed in settings.imported.iter().filter(|path| !imported.contains(path)) {
            let _ = fs::remove_file(self.copy_path(removed));
        }
        settings.imported = imported;
        let in_use = settings.mode == GeoMode::Imported;
        drop(settings);
        if in_use {
            self.refresh();
        }
    }

    pub fn refresh(self: &Arc<Self>) {
        let geo = Arc::clone(self);
        std::thread::spawn(move || geo.load());
    }

    fn load(&self) {
        let _queue = self.loading.lock().unwrap_or_else(PoisonError::into_inner);
        let (mode, imported) = {
            let mut settings = self.settings.lock().unwrap();
            if settings.mode == GeoMode::Off {
                return;
            }
            settings.status = "Checking…".to_string();
            (settings.mode, settings.imported.clone())
        };

        let loaded = match mode {
            GeoMode::Off => return,
            GeoMode::Free => self.fetch_all(&FREE_DATABASES, DAY),
            GeoMode::DbIp => self.fetch_all(&DBIP_DATABASES, MONTH),
            GeoMode::Imported => imported
                .iter()
                .map(|path| (path.display().to_string(), self.import(path)))
                .collect(),
        };

        let wanted = loaded.len();
        let mut databases = Vec::new();
        let mut failed = Vec::new();
        let mut imported_status = Vec::new();
        for (name, database) in loaded {
            if mode == GeoMode::Imported {
                imported_status.push(Imported::new(name.clone(), &database));
            }
            match database {
                Ok(database) => databases.push(database),
                Err(e) => {
                    eprintln!("geoip: {name}: {e:#}");
                    failed.push(name);
                }
            }
        }

        let mut settings = self.settings.lock().unwrap();
        if settings.mode != mode
            || (mode == GeoMode::Imported && settings.imported != imported)
        {
            return;
        }
        settings.status = if failed.is_empty() {
            if wanted == 0 { "No databases imported" } else { "Loaded" }.to_string()
        } else if mode == GeoMode::Imported {
            format!("{} of {wanted} couldn't be imported", failed.len())
        } else if failed.len() == wanted {
            "Unavailable".to_string()
        } else {
            format!("Partly loaded: {} unavailable", failed.join(", "))
        };
        if mode == GeoMode::Imported {
            settings.imported_status = imported_status;
        }
        *self.databases.write().unwrap() = databases;
        self.cache.lock().unwrap().clear();
    }

    fn fetch_all(
        &self,
        downloads: &[Download],
        max_age: Duration,
    ) -> Vec<(String, Result<Database>)> {
        downloads
            .iter()
            .map(|&(name, cached, file)| (name.into(), self.fetch(cached, file, max_age)))
            .collect()
    }

    fn fetch(&self, name: &str, file: &str, max_age: Duration) -> Result<Database> {
        let path = self.cache_dir.join(format!("{name}.mmdb"));
        if is_younger_than(&path, max_age)
            && let Ok(database) = Database::map(&path)
        {
            return Ok(database);
        }
        match download(&format!("{RELEASES}/{file}"))
            .and_then(|bytes| save(&path, &bytes))
        {
            Ok(()) => Database::map(&path),
            Err(e) => Database::map(&path).map_err(|_| e),
        }
    }

    fn import(&self, original: &Path) -> Result<Database> {
        let copy = self.copy_path(original);
        let modified = |path: &Path| {
            fs::metadata(path).and_then(|metadata| metadata.modified()).ok()
        };
        let stale = match (modified(original), modified(&copy)) {
            (Some(original), Some(copy)) => original > copy,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => bail!("file not found"),
        };
        if stale {
            let bytes = fs::read(original).context("reading it")?;
            let bytes = decompress(bytes).context("decompressing it")?;
            save(&copy, &bytes)?;
        }
        Database::map(&copy)
    }

    fn copy_path(&self, original: &Path) -> PathBuf {
        // FNV-1a hash to identify
        let hash = original
            .as_os_str()
            .as_bytes()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
        let name = original.file_name().unwrap_or_default().to_string_lossy();
        let name = name.strip_suffix(".gz").unwrap_or(name.as_ref());
        self.import_dir.join(format!("{hash:016x}-{name}"))
    }

    pub fn lookup(&self, ip: IpAddr) -> Option<Geo> {
        if !is_public(ip) {
            return None;
        }
        if let Some(geo) = self.cache.lock().unwrap().get(&ip) {
            return geo.clone();
        }

        self.pages_touched.store(true, Ordering::Relaxed);
        let found: Vec<Geo> = self
            .databases
            .read()
            .unwrap()
            .iter()
            .map(|database| database.geo(ip))
            .collect();
        let geo = merge(found);
        let mut cache = self.cache.lock().unwrap();
        if cache.len() > 50_000 {
            cache.clear();
        }
        cache.insert(ip, geo.clone());
        geo
    }

    pub fn keep_only(&self, ips: &HashSet<IpAddr>) {
        self.cache.lock().unwrap().retain(|ip, _| ips.contains(ip));
    }

    /// prevent memory build up
    pub fn release_pages(&self) {
        if !self.pages_touched.swap(false, Ordering::Relaxed) {
            return;
        }
        for database in self.databases.read().unwrap().iter() {
            database.release_pages();
        }
    }
}

/// combine user imported database lookups
fn merge(found: Vec<Geo>) -> Option<Geo> {
    let detail = |geo: &Geo| {
        if geo.city.is_some() {
            3
        } else if geo.state.is_some() {
            2
        } else if geo.country.is_some() {
            1
        } else {
            0
        }
    };
    let nowhere = Geo::default();
    let mut place = &nowhere;
    for geo in &found {
        if detail(geo) > detail(place) {
            place = geo;
        }
    }
    let geo = Geo {
        country: place
            .country
            .clone()
            .or_else(|| found.iter().find_map(|geo| geo.country.clone())),
        org: found.iter().find_map(|geo| geo.org.clone()),
        city: place.city.clone(),
        state: place.state.clone(),
    };
    let known = geo.country.is_some()
        || geo.org.is_some()
        || geo.city.is_some()
        || geo.state.is_some();
    known.then_some(geo)
}

// test database detail on these ip addrs
const TEST_IPS: [IpAddr; 4] = [
    IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
    IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
];

struct Database {
    reader: Reader<Contents>,
    // when mmap'd, only what's needed is loaded from the database
    mapping: Option<Arc<Mmap>>,
}

enum Contents {
    Memory(Vec<u8>),
    Mapped(Arc<Mmap>),
}

impl AsRef<[u8]> for Contents {
    fn as_ref(&self) -> &[u8] {
        match self {
            Contents::Memory(bytes) => bytes,
            Contents::Mapped(mapping) => mapping,
        }
    }
}

impl Database {
    fn read(path: &Path) -> Result<Database> {
        let bytes =
            fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let reader = Reader::from_source(Contents::Memory(bytes))
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(Database { reader, mapping: None })
    }

    fn map(path: &Path) -> Result<Database> {
        let file =
            File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mapping = match unsafe { Mmap::map(&file) } {
            Ok(mapping) => Arc::new(mapping),
            Err(e) => {
                eprintln!("geoip: can't map {}, reading it instead: {e}", path.display());
                return Database::read(path);
            }
        };
        let reader = Reader::from_source(Contents::Mapped(Arc::clone(&mapping)))
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(Database { reader, mapping: Some(mapping) })
    }

    fn geo(&self, ip: IpAddr) -> Geo {
        let Ok(found) = self.reader.lookup(ip) else {
            return Geo::default();
        };

        let text = |path: &[PathElement]| {
            found
                .decode_path::<String>(path)
                .ok()
                .flatten()
                .filter(|text| !text.is_empty())
        };
        let first = |paths: &[&[PathElement]]| paths.iter().find_map(|path| text(path));

        let country = first(&[
            &path!["country", "iso_code"],
            &path!["country_code"],
            &path!["registered_country", "iso_code"],
        ])
        .or_else(|| text(&path!["country"]).filter(|country| is_country_code(country)));

        let city = first(&[
            &path!["city", "names", "en"],
            &path!["city", "names", "es"],
            &path!["city", "names", "fr"],
            &path!["city", "names", "de"],
            &path!["city", "names", "pt-BR"],
            &path!["city", "names", "ru"],
            &path!["city", "names", "ja"],
            &path!["city", "names", "zh-CN"],
            &path!["city", "name"],
            &path!["city"],
            &path!["city_name"],
        ]);

        let state = first(&[
            &path!["subdivisions", 0, "iso_code"],
            &path!["subdivisions", 0, "names", "en"],
            &path!["state1"],
            &path!["region"],
            &path!["region_name"],
            &path!["state"],
        ])
        .filter(|_| country.as_deref() == Some("US"))
        .and_then(|name_or_code| us_state(&name_or_code));

        let org = first(&[
            &path!["autonomous_system_organization"],
            &path!["traits", "autonomous_system_organization"],
            &path!["as_name"],
            &path!["isp"],
            &path!["traits", "isp"],
            &path!["organization"],
            &path!["traits", "organization"],
        ])
        .or_else(|| text(&path!["asn"]).and_then(|_| text(&path!["name"])));
        Geo { country, org, city, state }
    }

    fn kind(&self) -> String {
        self.reader.metadata().database_type.clone()
    }

    fn contents(&self) -> String {
        let found: Vec<Geo> = TEST_IPS.iter().map(|&ip| self.geo(ip)).collect();
        let has = |field: fn(&Geo) -> bool| found.iter().any(field);
        let fields = [
            ("country", has(|geo| geo.country.is_some())),
            ("state", has(|geo| geo.state.is_some())),
            ("city", has(|geo| geo.city.is_some())),
            ("network", has(|geo| geo.org.is_some())),
        ];
        let names: Vec<&str> =
            fields.into_iter().filter(|(_, has)| *has).map(|(name, _)| name).collect();
        names.join(", ")
    }

    fn release_pages(&self) {
        let Some(mapping) = &self.mapping else {
            return;
        };
        unsafe {
            libc::madvise(
                mapping.as_ptr() as *mut libc::c_void,
                mapping.len(),
                libc::MADV_DONTNEED,
            );
        }
    }
}

fn is_country_code(text: &str) -> bool {
    text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn download(url: &str) -> Result<Vec<u8>> {
    let output = Command::new("curl")
        .args(["-fsSL", "--connect-timeout", "30"])
        .args(["--speed-limit", "1024", "--speed-time", "60", url])
        .output()
        .context("failed to spawn curl")?;
    ensure!(
        output.status.success(),
        "curl exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    decompress(output.stdout)
}

/// for gzipped files
fn decompress(bytes: Vec<u8>) -> Result<Vec<u8>> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(bytes);
    }
    let mut decompressed = Vec::new();
    flate2::read::GzDecoder::new(&bytes[..]).read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn save(path: &Path, bytes: &[u8]) -> Result<()> {
    Reader::from_source(bytes).map_err(|_| anyhow!("not an MMDB database"))?;
    let dir = path.parent().context("copy path has no directory")?;
    fs::create_dir_all(dir)?;
    let temporary = path.with_extension("mmdb.tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn is_younger_than(path: &Path, age: Duration) -> bool {
    let modified = fs::metadata(path).and_then(|metadata| metadata.modified());
    modified.is_ok_and(|time| time.elapsed().unwrap_or(Duration::MAX) < age)
}

/// copied from std unstable functions
fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.octets()[0] == 0 // "This network"
            || ip.is_private()
            || ip.octets()[0] == 100 && (ip.octets()[1] & 0b1100_0000 == 0b0100_0000)
            || ip.is_loopback()
            || ip.is_link_local()
            // addresses reserved for future protocols (`192.0.0.0/24`)
            // .9 and .10 are documented as globally reachable so they're excluded
            || (
                ip.octets()[0] == 192 && ip.octets()[1] == 0 && ip.octets()[2] == 0
                && ip.octets()[3] != 9 && ip.octets()[3] != 10
            )
            || ip.is_documentation()
            || ip.octets()[0] == 198 && (ip.octets()[1] & 0xfe) == 18
            || ip.octets()[0] & 240 == 240 && !ip.is_broadcast()
            || ip.is_broadcast())
        }
        IpAddr::V6(ip) => {
            !(ip.is_unspecified()
            || ip.is_loopback()
            // IPv4-mapped Address (`::ffff:0:0/96`)
            || matches!(ip.segments(), [0, 0, 0, 0, 0, 0xffff, _, _])
            // IPv4-IPv6 Translat. (`64:ff9b:1::/48`)
            || matches!(ip.segments(), [0x64, 0xff9b, 1, _, _, _, _, _])
            // Discard-Only Address Block (`100::/64`)
            || matches!(ip.segments(), [0x100, 0, 0, 0, _, _, _, _])
            // IETF Protocol Assignments (`2001::/23`)
            || (matches!(ip.segments(), [0x2001, b, _, _, _, _, _, _] if b < 0x200)
                && !(
                    // Port Control Protocol Anycast (`2001:1::1`)
                    u128::from_be_bytes(ip.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0001
                    // Traversal Using Relays around NAT Anycast (`2001:1::2`)
                    || u128::from_be_bytes(ip.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0002
                    // AMT (`2001:3::/32`)
                    || matches!(ip.segments(), [0x2001, 3, _, _, _, _, _, _])
                    // AS112-v6 (`2001:4:112::/48`)
                    || matches!(ip.segments(), [0x2001, 4, 0x112, _, _, _, _, _])
                    // ORCHIDv2 (`2001:20::/28`)
                    // Drone Remote ID Protocol Entity Tags (DETs) Prefix (`2001:30::/28`)`
                    || matches!(ip.segments(), [0x2001, b, _, _, _, _, _, _] if b >= 0x20 && b <= 0x3F)
                ))
            // 6to4 (`2002::/16`) – it's not explicitly documented as globally reachable,
            // IANA says N/A.
            || matches!(ip.segments(), [0x2002, _, _, _, _, _, _, _])
            || matches!(ip.segments(), [0x2001, 0xdb8, ..] | [0x3fff, 0..=0x0fff, ..])
            // Segment Routing (SRv6) SIDs (`5f00::/16`)
            || matches!(ip.segments(), [0x5f00, ..])
            || ip.is_unique_local()
            || ip.is_unicast_link_local())
        }
    }
}

fn us_state(name_or_code: &str) -> Option<String> {
    let (_, code) = US_STATES
        .iter()
        .find(|&&(name, code)| name == name_or_code || code == name_or_code)?;
    Some(code.to_string())
}

const US_STATES: [(&str, &str); 51] = [
    ("Alabama", "AL"),
    ("Alaska", "AK"),
    ("Arizona", "AZ"),
    ("Arkansas", "AR"),
    ("California", "CA"),
    ("Colorado", "CO"),
    ("Connecticut", "CT"),
    ("Delaware", "DE"),
    ("District of Columbia", "DC"),
    ("Florida", "FL"),
    ("Georgia", "GA"),
    ("Hawaii", "HI"),
    ("Idaho", "ID"),
    ("Illinois", "IL"),
    ("Indiana", "IN"),
    ("Iowa", "IA"),
    ("Kansas", "KS"),
    ("Kentucky", "KY"),
    ("Louisiana", "LA"),
    ("Maine", "ME"),
    ("Maryland", "MD"),
    ("Massachusetts", "MA"),
    ("Michigan", "MI"),
    ("Minnesota", "MN"),
    ("Mississippi", "MS"),
    ("Missouri", "MO"),
    ("Montana", "MT"),
    ("Nebraska", "NE"),
    ("Nevada", "NV"),
    ("New Hampshire", "NH"),
    ("New Jersey", "NJ"),
    ("New Mexico", "NM"),
    ("New York", "NY"),
    ("North Carolina", "NC"),
    ("North Dakota", "ND"),
    ("Ohio", "OH"),
    ("Oklahoma", "OK"),
    ("Oregon", "OR"),
    ("Pennsylvania", "PA"),
    ("Rhode Island", "RI"),
    ("South Carolina", "SC"),
    ("South Dakota", "SD"),
    ("Tennessee", "TN"),
    ("Texas", "TX"),
    ("Utah", "UT"),
    ("Vermont", "VT"),
    ("Virginia", "VA"),
    ("Washington", "WA"),
    ("West Virginia", "WV"),
    ("Wisconsin", "WI"),
    ("Wyoming", "WY"),
];
