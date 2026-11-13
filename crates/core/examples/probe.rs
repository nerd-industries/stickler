//! Hardware probe for the P21.
//! Usage: probe scan | pair <MAC> | info | selftest | orient

use nelko_core::config::Config;
use nelko_core::discovery;
use nelko_core::lock::{DEFAULT_LOCK_WAIT, PrinterLock};
use nelko_core::printer::Printer;
use nelko_core::transport::RfcommTransport;
use std::time::Duration;

type R = Result<(), Box<dyn std::error::Error>>;

async fn open(cfg: &Config) -> Result<(PrinterLock, Printer<RfcommTransport>), Box<dyn std::error::Error>> {
    let mac = cfg.printer_mac.clone().ok_or("no printer saved; run `probe pair <MAC>` first")?;
    let lock = PrinterLock::acquire(DEFAULT_LOCK_WAIT)?;
    let t = RfcommTransport::connect(&mac, Duration::from_secs(10)).await?;
    Ok((lock, Printer::new(t)))
}

#[tokio::main]
async fn main() -> R {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cfg = Config::load()?;
    match args.first().map(String::as_str) {
        Some("scan") => {
            for p in discovery::discover(Duration::from_secs(12)).await? {
                println!("{}  {}  paired={}", p.mac, p.name, p.paired);
            }
        }
        Some("pair") => {
            let mac = args.get(1).ok_or("usage: probe pair <MAC>")?;
            discovery::pair_and_trust(mac).await?;
            cfg.printer_mac = Some(mac.clone());
            cfg.printer_name = Some("P21".into());
            cfg.save()?;
            println!("paired and saved {mac}");
        }
        Some("info") => {
            let (_lock, mut p) = open(&cfg).await?;
            println!("status:  {:?}", p.status().await?);
            println!("battery: {:?}", p.battery().await?);
            println!("config:  {:?}", p.config().await?);
            println!("roll:    {:?}", p.roll().await?);
        }
        Some("selftest") => {
            let (_lock, mut p) = open(&cfg).await?;
            p.self_test().await?;
        }
        _ => eprintln!("usage: probe scan | pair <MAC> | info | selftest"),
    }
    Ok(())
}
