//! Hardware probe for the P21.
//! Usage: probe scan | pair <MAC> | info | selftest | orient | text <lines..> | preview <lines..>

use nelko_core::config::Config;
use nelko_core::discovery;
use nelko_core::lock::{DEFAULT_LOCK_WAIT, PrinterLock};
use nelko_core::geometry::print_area;
use nelko_core::printer::Printer;
use nelko_core::protocol::PrintJob;
use nelko_core::render::text::{TextStyle, draw_text_fit};
use nelko_core::render::{Canvas, finish};
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
        Some(cmd @ ("orient" | "text" | "preview")) => {
            // text/preview: probe text "line 1" ["line 2"]; preview writes label.png
            let lines: Vec<&str> = if cmd == "orient" { vec!["◀ TOP EDGE"] } else { args[1..].iter().map(String::as_str).collect() };
            let size = cfg.last_label.unwrap_or("14x40".parse()?);
            let mut c = Canvas::new(&print_area(size));
            let b = c.bounds();
            c.stroke_round_rect(b.inset(1.5), 10.0, 3.0);
            let style = TextStyle { bold: true, ..Default::default() };
            draw_text_fit(&mut c, b.inset(8.0), &lines, &style)?;
            let label = finish(c, size, 128);
            if cmd == "preview" {
                std::fs::write("label.png", label.preview_png(2))?;
                println!("wrote label.png ({size})");
                return Ok(());
            }
            let job = PrintJob { size, gap_mm: 5, density: cfg.density, copies: 1, bitmap: label.to_print_bitmap(cfg.rotation) };
            let (_lock, mut p) = open(&cfg).await?;
            p.print(&job).await?;
            println!("printed");
        }
        _ => eprintln!("usage: probe scan | pair <MAC> | info | selftest | orient | text <lines..> | preview <lines..>"),
    }
    Ok(())
}
