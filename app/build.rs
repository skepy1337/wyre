use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};
use std::fs;
use std::path::PathBuf;

fn main() {
    let module = QmlModule::new("dev.skepy.wyre")
        .qml_files([
            "qml/Main.qml",
            "qml/Toolbar.qml",
            "qml/ConnectionList.qml",
            "qml/TreeRow.qml",
            "qml/GroupRow.qml",
            "qml/ProcessRow.qml",
            "qml/ConnectionRow.qml",
            "qml/Chevron.qml",
            "qml/NumberCell.qml",
            "qml/ProtocolBadge.qml",
            "qml/DetailPane.qml",
            "qml/AboutDialog.qml",
            "qml/GeoSetupDialog.qml",
            "qml/ImportedDatabasesDialog.qml",
        ])
        .qml_file(QmlFile::from("qml/singletons/Theme.qml").singleton(true))
        .qml_file(QmlFile::from("qml/singletons/Rows.qml").singleton(true));

    let mut flags: Vec<PathBuf> = fs::read_dir("flags")
        .expect("read flags/")
        .map(|entry| entry.expect("read flags/").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
        .collect();
    flags.sort();
    println!("cargo::rerun-if-changed=flags");

    CxxQtBuilder::new_qml_module(module)
        .files(["bridge.rs"])
        .qrc_resources(flags)
        .build();
}
