use std::collections::HashSet;
use std::env;

use bluer::{Adapter, AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport};
use bluer_miflora::Miflora;
use futures::{pin_mut, StreamExt};

// async fn query_all_device_properties(adapter: &Adapter, addr: Address) -> bluer::Result<()> {
//     let device = adapter.device(addr)?;
//     let props = device.all_properties().await?;
//     for prop in props {
//         println!("    {:?}", &prop);
//     }
//     Ok(())
// }

pub async fn handle(adapter: Adapter, addr: Address) -> anyhow::Result<()> {
    let miflora = Miflora::from_adapter(&adapter, addr)?;
    miflora.try_connect(5).await?;
    println!("info:   {:?}", miflora.read_system().await?);
    println!("values: {:?}", miflora.read_historical_values().await?);
    miflora.try_disconnect(5).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addresses: HashSet<_> = env::args()
        .filter_map(|arg| arg.parse::<Address>().ok())
        .collect();

    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    println!(
        "Discovering devices using Bluetooth adapter {}\n",
        adapter.name()
    );
    adapter.set_powered(true).await?;

    let filter = DiscoveryFilter {
        transport: DiscoveryTransport::Le,
        pattern: Some("Flower care".into()),
        ..Default::default()
    };
    adapter.set_discovery_filter(filter).await?;
    println!(
        "Using discovery filter:\n{:#?}\n\n",
        adapter.discovery_filter().await
    );

    let device_events = adapter.discover_devices().await?;
    pin_mut!(device_events);

    while let Some(event) = device_events.next().await {
        match event {
            AdapterEvent::DeviceAdded(addr) => {
                let device = adapter.device(addr)?;
                let name = device.name().await?;
                println!("device {addr} discovered {name:?}");
                if addresses.contains(&addr) {
                    if let Err(err) = handle(adapter.clone(), addr).await {
                        println!("=> something wend wrong with {addr}: {err:?}");
                    }
                }
            }
            AdapterEvent::DeviceRemoved(addr) => {
                println!("device {addr} disappeared");
            }
            _ => {}
        }
    }

    Ok(())
}
