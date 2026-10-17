mod bridge;
mod geoip;
mod rows;
mod settings;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let (Some(mut app), Some(engine)) = (app.as_mut(), engine.as_mut()) else {
        return;
    };
    app.as_mut().set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    QGuiApplication::set_desktop_file_name(&QString::from("dev.skepy.wyre"));
    engine.load(&QUrl::from("qrc:/qt/qml/dev/skepy/wyre/qml/Main.qml"));
    app.exec();
}
