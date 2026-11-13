//! Finding and pairing P21 printers through BlueZ.

use crate::error::PrinterError;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FoundPrinter {
    pub mac: String,
    pub name: String,
    pub paired: bool,
}

/// P21s advertise as "P21" (verified on hardware); allow suffixes like "P21_xxxx".
pub fn is_p21_name(name: &str) -> bool {
    name.to_ascii_uppercase().starts_with("P21")
}

fn bt(e: bluer::Error) -> PrinterError {
    PrinterError::Bluetooth(e.to_string())
}

async fn adapter() -> Result<bluer::Adapter, PrinterError> {
    let session = bluer::Session::new().await.map_err(bt)?;
    let adapter = session.default_adapter().await.map_err(|_| {
        PrinterError::Bluetooth("No Bluetooth adapter found. Is Bluetooth turned on?".into())
    })?;
    adapter.set_powered(true).await.map_err(bt)?;
    Ok(adapter)
}

async fn describe(adapter: &bluer::Adapter, addr: bluer::Address) -> Option<FoundPrinter> {
    let dev = adapter.device(addr).ok()?;
    let name = dev.name().await.ok().flatten()?;
    if !is_p21_name(&name) {
        return None;
    }
    let paired = dev.is_paired().await.unwrap_or(false);
    Some(FoundPrinter { mac: addr.to_string(), name, paired })
}

/// Scan for `timeout` and return every P21 seen, plus already-known ones.
pub async fn discover(timeout: Duration) -> Result<Vec<FoundPrinter>, PrinterError> {
    let adapter = adapter().await?;
    let mut found = BTreeMap::new();
    for addr in adapter.device_addresses().await.map_err(bt)? {
        if let Some(p) = describe(&adapter, addr).await {
            found.insert(p.mac.clone(), p);
        }
    }
    let filter = bluer::DiscoveryFilter { transport: bluer::DiscoveryTransport::BrEdr, ..Default::default() };
    adapter.set_discovery_filter(filter).await.map_err(bt)?;
    let mut events = adapter.discover_devices().await.map_err(bt)?;
    let _ = tokio::time::timeout(timeout, async {
        while let Some(ev) = events.next().await {
            if let bluer::AdapterEvent::DeviceAdded(addr) = ev {
                if let Some(p) = describe(&adapter, addr).await {
                    found.insert(p.mac.clone(), p);
                }
            }
        }
    })
    .await;
    Ok(found.into_values().collect())
}

/// Pair (no PIN needed on the P21) and trust, so later connects need no prompt.
pub async fn pair_and_trust(mac: &str) -> Result<(), PrinterError> {
    let addr: bluer::Address =
        mac.parse().map_err(|_| PrinterError::Bluetooth(format!("'{mac}' isn't a Bluetooth address")))?;
    let adapter = adapter().await?;
    let dev = adapter.device(addr).map_err(bt)?;
    if !dev.is_paired().await.map_err(bt)? {
        dev.pair().await.map_err(bt)?;
    }
    dev.set_trusted(true).await.map_err(bt)?;
    Ok(())
}
