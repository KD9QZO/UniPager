#[cfg(hid_ptt)]
extern crate hidapi;
extern crate raspi;
extern crate serde;
extern crate serial;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
extern crate chrono;
extern crate failure;
extern crate futures_core;
extern crate futures_util;
extern crate hyper;
extern crate reqwest;
extern crate tokio;
extern crate tungstenite;

#[macro_use]
mod telemetry;
mod config;
mod connection;
mod core;
mod event;
mod frontend;
mod logging;
mod message;
mod pocsag;
mod queue;
mod scheduler;
mod timeslots;
mod transmitter;

use std::fs::File;
use std::io::Read;

use async_std::prelude::*;
use tokio::runtime::Runtime;

fn print_version() {
    println!("UniPager {}", env!("CARGO_PKG_VERSION"));
    println!("Copyright (c) 2017-2021 RWTH Amateurfunkgruppe\n");
    println!("This program comes with ABSOLUTELY NO WARRANTY.");
    println!("This is free software, and you are welcome to redistribute");
    println!("and modify it under the conditions of the GNU GPL v3 or later.");
    println!("<https://www.gnu.org/licenses/gpl-3.0.txt>\n");
}

fn main() {
    print_version();

    let pass = File::open("password")
        .and_then(|mut f| {
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            Ok(s)
        })
        .map(|s| s.trim().to_owned())
        .map_err(|_| eprintln!("Failed to load password file."))
        .ok();

    let mut runtime = Runtime::new().unwrap();

    let config = config::get();
    let event_handler = event::start(&runtime);

    logging::init(event_handler.clone());
    scheduler::start(config.clone(), event_handler.clone());
    telemetry::start(&runtime, event_handler.clone());
    frontend::websocket::start(&runtime, pass, event_handler.clone());
    frontend::http::start(&runtime, event_handler.clone());
    if config.master.standalone_mode {
        info!(
            "Starting up in standalone mode. Connection to server is skipped."
        )
    } else {
        timeslots::start(&runtime, event_handler.clone());
        connection::start(&runtime, &config, event_handler.clone());
        core::start(&runtime, &config, event_handler.clone());
    }

    runtime.block_on(async move {
        let (tx, mut rx) = event::channel();
        event_handler.publish(event::Event::RegisterMain(tx));

        while let Some(event) = rx.next().await {
            match event {
                event::Event::Shutdown => {
                    return;
                }
                _ => {}
            }
        }
    });

    info!("Terminating... 73!");
}
