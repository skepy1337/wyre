use std::fs;
use std::ops::RangeInclusive;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

const NO_ISOLATION: &str = "/etc/wyre/no-user-isolation";
const HOST_ROOT: &str = "/run/host/root";

pub struct Users {
    people: RangeInclusive<u32>,
    isolated: bool,
}

impl Users {
    pub fn load() -> Users {
        let defs = fs::read_to_string("/etc/login.defs").unwrap_or_default();
        let setting = |name: &str, default: u32| {
            defs.lines()
                .find_map(|line| {
                    let mut words = line.split_whitespace();
                    (words.next() == Some(name)).then(|| words.next()?.parse().ok())?
                })
                .unwrap_or(default)
        };
        let people = setting("UID_MIN", 1000)..=setting("UID_MAX", 60000);
        let mut users = Users { people, isolated: true };
        users.refresh();
        users
    }

    pub fn refresh(&mut self) {
        let on_host = Path::new(HOST_ROOT).join(NO_ISOLATION.trim_start_matches('/'));
        let isolated = ![Path::new(NO_ISOLATION), &on_host].into_iter().any(made_by_root);
        if isolated != self.isolated {
            if isolated {
                println!("User isolation is on");
            } else {
                println!("User isolation is off, as {NO_ISOLATION} exists");
            }
        }
        self.isolated = isolated;
    }

    pub fn can_see(&self, client: u32, pid: u32, owner: u32) -> bool {
        !self.isolated
            || client == 0
            || pid == 0
            || owner == client
            || !self.people.contains(&owner)
    }
}

fn made_by_root(path: &Path) -> bool {
    let Ok(file) = fs::symlink_metadata(path) else {
        return false;
    };
    file.is_file()
        && file.uid() == 0
        && path.ancestors().skip(1).all(|dir| {
            fs::metadata(dir).is_ok_and(|dir| dir.uid() == 0 && dir.mode() & 0o022 == 0)
        })
}
