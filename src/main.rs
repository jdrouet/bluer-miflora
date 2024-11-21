use bluer::{AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport};
use bluer_miflora::handle;
use futures::{pin_mut, StreamExt};
use std::{collections::HashSet, env};

// async fn query_all_device_properties(adapter: &Adapter, addr: Address) -> bluer::Result<()> {
//     let device = adapter.device(addr)?;
//     let props = device.all_properties().await?;
//     for prop in props {
//         println!("    {:?}", &prop);
//     }
//     Ok(())
// }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addresses: HashSet<_> = env::args()
        .filter_map(|arg| arg.parse::<Address>().ok())
        .collect();

    env_logger::init();
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

    // let mut all_change_events = SelectAll::new();

    // loop {
    //     tokio::select! {
    //         Some(device_event) = device_events.next() => {
    //             match device_event {
    //                 AdapterEvent::DeviceAdded(addr) => {
    //                     if !filter_addr.is_empty() && !filter_addr.contains(&addr) {
    //                         continue;
    //                     }

    //                     println!("Device added: {addr}");
    //                     if let Err(err) = bluer_miflora::handle(&adapter, addr, None).await {
    //                         println!("    Error: {}", &err);
    //                     }

    //                     let device = adapter.device(addr)?;
    //                     let change_events = device.events().await?.map(move |evt| (addr, evt));
    //                     all_change_events.push(change_events);
    //                 }
    //                 AdapterEvent::DeviceRemoved(addr) => {
    //                     println!("Device removed: {addr}");
    //                 }
    //                 _ => (),
    //             }
    //             println!();
    //         }
    //         Some((addr, DeviceEvent::PropertyChanged(property))) = all_change_events.next() => {
    //             println!("Device changed: {addr}");
    //             println!("    {property:?}");

    //             if let Err(err) = bluer_miflora::handle(&adapter, addr, Some(property)).await {
    //                 println!("    Error: {}", &err);
    //             }
    //         }
    //         else => break
    //     }
    // }

    Ok(())
}
