use crate::geoip::{GeoDb, GeoMode, Imported};
use crate::rows::{self, View};
use crate::settings::{self, Settings, ViewSettings};
use common::{ProcessGroup, SOCKET_PATH};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::net::IpAddr;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QString, snapshot_json)]
        #[qproperty(QString, total_recv_str)]
        #[qproperty(QString, total_send_str)]
        #[qproperty(i32, total_conns)]
        #[qproperty(QString, error)]
        #[qproperty(bool, connected)]
        #[qproperty(QString, sort_by)]
        #[qproperty(bool, sort_desc)]
        #[qproperty(QString, conn_sort_by)]
        #[qproperty(bool, conn_sort_desc)]
        #[qproperty(bool, show_tcp)]
        #[qproperty(bool, show_udp)]
        #[qproperty(bool, show_icmp)]
        #[qproperty(bool, hide_dns)]
        #[qproperty(bool, hide_empty)]
        #[qproperty(f64, zoom)]
        #[qproperty(QString, geo_status)]
        #[qproperty(QString, geo_mode)]
        #[qproperty(bool, geo_choice_needed)]
        #[qproperty(QString, imported_json)]
        type WyreModel = super::WyreModelRust;
    }

    extern "RustQt" {
        #[qinvokable]
        fn connection_json(
            self: &WyreModel,
            pid: i32,
            proto: &QString,
            local: &QString,
            remote: &QString,
        ) -> QString;

        #[qinvokable]
        fn fetch(self: Pin<&mut WyreModel>);

        #[qinvokable]
        fn set_sort(self: Pin<&mut WyreModel>, key: &QString, desc: bool);

        #[qinvokable]
        fn set_conn_sort(self: Pin<&mut WyreModel>, key: &QString, desc: bool);

        #[qinvokable]
        fn set_search(self: Pin<&mut WyreModel>, query: &QString);

        #[qinvokable]
        fn toggle_group(self: Pin<&mut WyreModel>, name: &QString);

        #[qinvokable]
        fn toggle_proc(self: Pin<&mut WyreModel>, pid: i32);

        #[qinvokable]
        fn toggle_protocol(self: Pin<&mut WyreModel>, proto: &QString);

        #[qinvokable]
        fn toggle_dns(self: Pin<&mut WyreModel>);

        #[qinvokable]
        fn toggle_empty(self: Pin<&mut WyreModel>);

        #[qinvokable]
        fn zoom_in(self: Pin<&mut WyreModel>);

        #[qinvokable]
        fn zoom_out(self: Pin<&mut WyreModel>);

        #[qinvokable]
        fn reset_zoom(self: Pin<&mut WyreModel>);

        #[qinvokable]
        fn select_geo_mode(self: Pin<&mut WyreModel>, mode: &QString);

        #[qinvokable]
        fn refresh_geo(self: Pin<&mut WyreModel>);

        #[qinvokable]
        fn import_database(self: Pin<&mut WyreModel>, file: &QUrl);

        #[qinvokable]
        fn remove_database(self: Pin<&mut WyreModel>, index: i32);

        #[qinvokable]
        fn trim_memory(self: Pin<&mut WyreModel>);
    }

    impl cxx_qt::Threading for WyreModel {}
}

pub struct WyreModelRust {
    groups: Vec<ProcessGroup>,
    search: String,
    expanded_groups: HashMap<String, bool>,
    expanded_processes: HashMap<i32, bool>,
    geo: Arc<GeoDb>,
    settings: Settings,

    snapshot_json: QString,
    total_recv_str: QString,
    total_send_str: QString,
    total_conns: i32,
    error: QString,
    connected: bool,
    sort_by: QString,
    sort_desc: bool,
    conn_sort_by: QString,
    conn_sort_desc: bool,
    show_tcp: bool,
    show_udp: bool,
    show_icmp: bool,
    hide_dns: bool,
    hide_empty: bool,
    zoom: f64,
    geo_status: QString,
    geo_mode: QString,
    geo_choice_needed: bool,
    imported_json: QString,
}

impl Default for WyreModelRust {
    fn default() -> Self {
        let settings = settings::load();
        let geo = GeoDb::new();
        geo.set_imported(settings.imported.clone());
        if let Some(mode) = settings.geo_mode {
            geo.set_mode(mode);
        }
        let status = geo.status();

        WyreModelRust {
            groups: Vec::new(),
            search: String::new(),
            expanded_groups: HashMap::new(),
            expanded_processes: HashMap::new(),

            snapshot_json: QString::from("[]"),
            total_recv_str: QString::from("0 B"),
            total_send_str: QString::from("0 B"),
            total_conns: 0,
            error: QString::from(""),
            connected: false,
            sort_by: QString::from("name"),
            sort_desc: false,
            conn_sort_by: QString::from("download"),
            conn_sort_desc: true,
            show_tcp: settings.view.show_tcp,
            show_udp: settings.view.show_udp,
            show_icmp: settings.view.show_icmp,
            hide_dns: settings.view.hide_dns,
            hide_empty: settings.view.hide_empty,
            zoom: settings.view.zoom,
            geo_status: QString::from(&status.status),
            geo_mode: QString::from(settings.geo_mode.unwrap_or(GeoMode::Off).as_str()),
            geo_choice_needed: settings.geo_mode.is_none(),
            imported_json: QString::from(&imported_json(&status.imported)),

            geo,
            settings,
        }
    }
}

enum Reply {
    Snapshot(Vec<ProcessGroup>),
    BadData(String),
    Unreachable(String),
}

fn read_snapshot() -> Result<String, String> {
    let mut stream =
        UnixStream::connect(SOCKET_PATH).map_err(|e| format!("connect: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| format!("set_read_timeout: {e}"))?;
    let mut json = String::new();
    stream.read_to_string(&mut json).map_err(|e| format!("read: {e}"))?;
    Ok(json)
}

impl ffi::WyreModel {
    fn fetch(mut self: Pin<&mut Self>) {
        self.as_mut().update_geo_status();

        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let reply = match read_snapshot() {
                Ok(json) => match serde_json::from_str(&json) {
                    Ok(groups) => Reply::Snapshot(groups),
                    Err(e) => Reply::BadData(e.to_string()),
                },
                Err(e) => Reply::Unreachable(e),
            };
            let _ = qt_thread.queue(move |model| {
                model.receive(reply);
            });
        });
    }

    fn receive(mut self: Pin<&mut Self>, reply: Reply) {
        let groups = match reply {
            Reply::Snapshot(groups) => groups,
            Reply::BadData(e) => {
                self.set_error(QString::from(&e));
                return;
            }
            Reply::Unreachable(e) => {
                self.as_mut().set_error(QString::from(&e));
                self.set_connected(false);
                return;
            }
        };

        let names: HashSet<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        let processes = groups.iter().flat_map(|g| &g.processes);
        let pids: HashSet<i32> = processes.clone().map(|p| p.pid).collect();
        let ips: HashSet<IpAddr> = processes
            .flat_map(|p| &p.connections)
            .map(|c| c.remote_address.ip())
            .collect();
        let mut rust = self.as_mut().rust_mut();
        rust.expanded_groups.retain(|name, _| names.contains(name.as_str()));
        rust.expanded_processes.retain(|pid, _| pids.contains(pid));
        rust.geo.keep_only(&ips);
        rust.groups = groups;

        self.as_mut().set_error(QString::from(""));
        self.as_mut().set_connected(true);
        self.update_all();
    }

    fn set_sort(mut self: Pin<&mut Self>, key: &QString, desc: bool) {
        self.as_mut().set_sort_by(key.clone());
        self.as_mut().set_sort_desc(desc);
        self.update_rows();
    }

    fn set_conn_sort(mut self: Pin<&mut Self>, key: &QString, desc: bool) {
        self.as_mut().set_conn_sort_by(key.clone());
        self.as_mut().set_conn_sort_desc(desc);
        self.update_rows();
    }

    fn set_search(mut self: Pin<&mut Self>, query: &QString) {
        self.as_mut().rust_mut().search = query.to_string().to_lowercase();
        self.update_rows();
    }

    fn connection_json(
        &self,
        pid: i32,
        proto: &QString,
        local: &QString,
        remote: &QString,
    ) -> QString {
        let rust = self.rust();
        let [proto, local, remote] = [proto, local, remote].map(QString::to_string);
        let groups = &rust.groups;
        let json = rows::connection(groups, pid, &proto, &local, &remote, &rust.geo);
        QString::from(json.as_deref().unwrap_or_default())
    }

    fn toggle_group(mut self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        let expanded = self.view().is_group_expanded(&name);
        self.as_mut().rust_mut().expanded_groups.insert(name, !expanded);
        self.update_rows();
    }

    fn toggle_proc(mut self: Pin<&mut Self>, pid: i32) {
        let groups = &self.rust().groups;
        let group = groups.iter().find(|g| g.processes.iter().any(|p| p.pid == pid));
        let group = group.map(|g| g.name.clone()).unwrap_or_default();
        let expanded = self.view().is_process_expanded(pid, &group);
        self.as_mut().rust_mut().expanded_processes.insert(pid, !expanded);
        self.update_rows();
    }

    fn toggle_protocol(mut self: Pin<&mut Self>, proto: &QString) {
        let (shown, set_shown): (bool, fn(Pin<&mut Self>, bool)) =
            match proto.to_string().as_str() {
                "tcp" => (*self.show_tcp(), Self::set_show_tcp),
                "udp" => (*self.show_udp(), Self::set_show_udp),
                "icmp" => (*self.show_icmp(), Self::set_show_icmp),
                _ => return,
            };
        set_shown(self.as_mut(), !shown);
        self.as_mut().save_view();
        self.update_all();
    }

    fn toggle_dns(mut self: Pin<&mut Self>) {
        let hide = !*self.hide_dns();
        self.as_mut().set_hide_dns(hide);
        self.as_mut().save_view();
        self.update_all();
    }

    fn toggle_empty(mut self: Pin<&mut Self>) {
        let hide = !*self.hide_empty();
        self.as_mut().set_hide_empty(hide);
        self.as_mut().save_view();
        self.update_all();
    }

    fn zoom_in(self: Pin<&mut Self>) {
        let zoom = settings::step_zoom(*self.zoom(), true);
        self.change_zoom(zoom);
    }

    fn zoom_out(self: Pin<&mut Self>) {
        let zoom = settings::step_zoom(*self.zoom(), false);
        self.change_zoom(zoom);
    }

    fn reset_zoom(self: Pin<&mut Self>) {
        self.change_zoom(1.0);
    }

    fn change_zoom(mut self: Pin<&mut Self>, zoom: f64) {
        if zoom != *self.zoom() {
            self.as_mut().set_zoom(zoom);
            self.save_view();
        }
    }

    fn save_view(mut self: Pin<&mut Self>) {
        let view = ViewSettings {
            show_tcp: *self.show_tcp(),
            show_udp: *self.show_udp(),
            show_icmp: *self.show_icmp(),
            hide_dns: *self.hide_dns(),
            hide_empty: *self.hide_empty(),
            zoom: *self.zoom(),
        };
        let mut rust = self.as_mut().rust_mut();
        rust.settings.view = view;
        settings::save(&rust.settings);
    }

    fn select_geo_mode(self: Pin<&mut Self>, mode: &QString) {
        if let Some(mode) = GeoMode::parse(&mode.to_string()) {
            self.change_geo_mode(mode);
        }
    }

    fn refresh_geo(self: Pin<&mut Self>) {
        self.rust().geo.refresh();
    }

    fn import_database(mut self: Pin<&mut Self>, file: &QUrl) {
        let path = PathBuf::from(file.to_local_file_or_default().to_string());
        let mut rust = self.as_mut().rust_mut();
        if !rust.settings.imported.contains(&path) {
            rust.settings.imported.push(path);
        }
        self.as_mut().change_imported();
        if self.rust().settings.geo_mode != Some(GeoMode::Imported) {
            self.change_geo_mode(GeoMode::Imported);
        }
    }

    fn remove_database(mut self: Pin<&mut Self>, index: i32) {
        let mut rust = self.as_mut().rust_mut();
        if let Ok(index) = usize::try_from(index)
            && index < rust.settings.imported.len()
        {
            rust.settings.imported.remove(index);
        }
        self.change_imported();
    }

    fn trim_memory(self: Pin<&mut Self>) {
        release_free_memory();
    }

    fn change_geo_mode(mut self: Pin<&mut Self>, mode: GeoMode) {
        let mut rust = self.as_mut().rust_mut();
        rust.settings.geo_mode = Some(mode);
        settings::save(&rust.settings);
        rust.geo.set_mode(mode);
        self.as_mut().set_geo_mode(QString::from(mode.as_str()));
        self.as_mut().set_geo_choice_needed(false);
        self.as_mut().update_geo_status();
        self.update_rows();
    }

    fn change_imported(mut self: Pin<&mut Self>) {
        let rust = self.as_mut().rust_mut();
        settings::save(&rust.settings);
        rust.geo.set_imported(rust.settings.imported.clone());
        self.as_mut().update_geo_status();
        self.update_rows();
    }

    fn update_geo_status(mut self: Pin<&mut Self>) {
        let status = self.rust().geo.status();
        self.as_mut().set_geo_status(QString::from(&status.status));
        self.as_mut().set_imported_json(QString::from(&imported_json(&status.imported)));
    }

    fn update_all(mut self: Pin<&mut Self>) {
        let totals = rows::totals(&self.rust().groups, &self.view());
        self.as_mut().set_total_recv_str(QString::from(&rows::format_bytes(totals.recv)));
        self.as_mut().set_total_send_str(QString::from(&rows::format_bytes(totals.send)));
        self.as_mut().set_total_conns(totals.conns as i32);
        self.update_rows();
    }

    fn update_rows(mut self: Pin<&mut Self>) {
        let json = rows::build(&self.rust().groups, &self.view(), &self.rust().geo);
        self.rust().geo.release_pages();
        self.as_mut().set_snapshot_json(QString::from(&json));
    }

    fn view(&self) -> View {
        let rust = self.rust();
        View {
            search: rust.search.clone(),
            show_tcp: *self.show_tcp(),
            show_udp: *self.show_udp(),
            show_icmp: *self.show_icmp(),
            hide_dns: *self.hide_dns(),
            hide_empty: *self.hide_empty(),
            sort_by: self.sort_by().to_string(),
            sort_desc: *self.sort_desc(),
            conn_sort_by: self.conn_sort_by().to_string(),
            conn_sort_desc: *self.conn_sort_desc(),
            expanded_groups: rust.expanded_groups.clone(),
            expanded_processes: rust.expanded_processes.clone(),
        }
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn release_free_memory() {
    unsafe { libc::malloc_trim(0) };
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn release_free_memory() {}

fn imported_json(imported: &[Imported]) -> String {
    serde_json::to_string(imported).expect("imported databases serialize")
}
