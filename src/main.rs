#![warn(
    rust_2018_idioms,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    clippy::explicit_iter_loop,
    clippy::semicolon_if_nothing_returned,
    clippy::match_wildcard_for_single_variants,
    clippy::inefficient_to_string,
    clippy::map_unwrap_or,
    clippy::implicit_clone,
    clippy::struct_excessive_bools,
    clippy::trivially_copy_pass_by_ref,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::doc_markdown,
    clippy::unused_async,
    clippy::default_trait_access,
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::dbg_macro,
    clippy::todo,
    clippy::print_stdout
)]

mod about;
mod application;
mod audio_device;
mod clock_time;
mod config;
mod core;
mod error_dialog;
mod inspector_page;
mod macros;
mod model;
mod player;
mod recognizer;
mod settings;
mod utils;
mod window;

use gettextrs::{gettext, LocaleCategory};
use gtk::{gio, glib};

use self::{
    application::Application,
    config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE},
};

pub struct Timer {
    start: std::time::Instant,
    name: String,
}

impl Timer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            start: std::time::Instant::now(),
            name: name.into(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        tracing::debug!("{} took {:?}", self.name, self.start.elapsed());
    }
}

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt::init();

    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Mousai"));

    gst::init().expect("Unable to start GStreamer");

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    model::db::ensure();

    let app = Application::new();
    app.run()
}
