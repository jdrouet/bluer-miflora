use std::collections::HashSet;
use std::env;

use bluer::{Adapter, AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport};
use bluer_miflora::Miflora;
use futures::{pin_mut, StreamExt};

fn enable_tracing() {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    if tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "miflora=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .is_err()
    {
        tracing::warn!("tracing already set");
    }
}

pub async fn handle(adapter: Adapter, addr: Address) -> anyhow::Result<()> {
    let miflora = Miflora::from_adapter(&adapter, addr)?;
    miflora.try_connect(5).await?;
    let system = miflora.read_system().await?;
    tracing::info!(message = "system information", address = %addr, battery = system.battery(), firmware = %system.firmware());
    let values = miflora.read_realtime_values().await?;
    tracing::info!(
        message = "realtime values",
        address = %addr,
        temperature = values.temperature(),
        brightness = values.brightness(),
        moisture = values.moisture(),
        conductivity = values.conductivity(),
    );
    miflora.try_disconnect(5).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    enable_tracing();

    let addresses: HashSet<_> = env::args()
        .filter_map(|arg| arg.parse::<Address>().ok())
        .collect();

    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    tracing::info!(
        "discovering devices using Bluetooth adapter {}",
        adapter.name()
    );
    adapter.set_powered(true).await?;

    adapter
        .set_discovery_filter(DiscoveryFilter {
            transport: DiscoveryTransport::Le,
            pattern: Some("Flower care".into()),
            ..Default::default()
        })
        .await?;

    let device_events = adapter.discover_devices().await?;
    pin_mut!(device_events);

    while let Some(event) = device_events.next().await {
        match event {
            AdapterEvent::DeviceAdded(addr) => {
                let device = adapter.device(addr)?;
                let name = device.name().await?;
                tracing::debug!(message = "device discovered", address = %addr, name = ?name);
                if addresses.contains(&addr) {
                    if let Err(err) = handle(adapter.clone(), addr).await {
                        tracing::warn!(message = "something went wrong", address = %addr, error = %err);
                    }
                }
            }
            AdapterEvent::DeviceRemoved(addr) => {
                tracing::debug!(message = "device disappeared", address = %addr);
            }
            _ => {}
        }
    }

    Ok(())
}
