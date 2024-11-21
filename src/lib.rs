use std::borrow::Cow;

use bluer::{
    gatt::{
        remote::{Characteristic, CharacteristicWriteRequest},
        WriteOp,
    },
    Adapter, Address, Device,
};

const SERVICE_DATA_ID: u16 = 49;
const CHARACTERISTIC_MODE_ID: u16 = 50;
const CHARACTERISTIC_DATA_ID: u16 = 52;
const CHARACTERISTIC_FIRMWARE_ID: u16 = 0x37;

const CMD_BLINK_LED: [u8; 2] = [0xfd, 0xff];
const CMD_REALTIME_ENABLE: [u8; 2] = [0xa0, 0x1f];
const REALTIME_DISABLE: [u8; 2] = [0xa0, 0x1f];

struct PayloadGenerator {
    inner: Option<u16>,
}

impl Default for PayloadGenerator {
    fn default() -> Self {
        Self { inner: Some(0) }
    }
}

impl Iterator for PayloadGenerator {
    type Item = [u8; 2];

    fn next(&mut self) -> Option<[u8; 2]> {
        match self.inner {
            Some(value) => {
                self.inner = value.checked_add_signed(1);
                Some(value.to_be_bytes())
            }
            None => None,
        }
    }
}

#[derive(Clone)]
struct System {
    inner: Vec<u8>,
}

impl From<Vec<u8>> for System {
    fn from(inner: Vec<u8>) -> Self {
        Self { inner }
    }
}

impl System {
    pub fn battery(&self) -> u8 {
        self.inner[0]
    }

    pub fn firmware(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.inner[2..])
    }
}

impl std::fmt::Debug for System {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(stringify!(System))
            .field("battery", &self.battery())
            .field("firmware", &self.firmware())
            .finish()
    }
}

/// Represents a real time entry of sensor values by parsing the byte array returned by the device.
///
/// The sensor returns 16 bytes in total.
/// It's unclear what the meaning of these bytes is beyond what is decoded in this method.
///
/// Semantics of the data (in little endian encoding):
/// bytes   0-1: temperature in 0.1 °C
/// byte      2: unknown
/// bytes   3-6: brightness in lux
/// byte      7: moisture in %
/// byted   8-9: conductivity in µS/cm
/// bytes 10-15: unknown
///
/// (source https://github.com/vrachieru/xiaomi-flower-care-api/blob/master/flowercare/reader.py#L138)
#[derive(Clone)]
struct RealtimeEntry {
    inner: Vec<u8>,
}

impl From<Vec<u8>> for RealtimeEntry {
    fn from(inner: Vec<u8>) -> Self {
        Self { inner }
    }
}

impl RealtimeEntry {
    pub fn temperature(&self) -> u16 {
        u16::from_le_bytes([self.inner[0], self.inner[1]])
    }

    pub fn brightness(&self) -> u32 {
        u32::from_le_bytes([self.inner[3], self.inner[4], self.inner[5], self.inner[6]])
    }

    pub fn moisture(&self) -> u8 {
        self.inner[7]
    }

    pub fn conductivity(&self) -> u16 {
        u16::from_le_bytes([self.inner[8], self.inner[9]])
    }
}

impl std::fmt::Debug for RealtimeEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(stringify!(RealTimeEntry))
            .field("temperature", &self.temperature())
            .field("brightness", &self.brightness())
            .field("moisture", &self.moisture())
            .field("conductivity", &self.conductivity())
            .finish()
    }
}

#[derive(Clone, Debug)]
struct Miflora {
    address: Address,
    device: Device,
}

impl Miflora {
    async fn characteristic(&self, char_id: u16) -> anyhow::Result<Characteristic> {
        let service = self.device.service(SERVICE_DATA_ID).await?;
        let char = service.characteristic(char_id).await?;
        Ok(char)
    }

    async fn read(&self, char_id: u16) -> anyhow::Result<Vec<u8>> {
        let char = self.characteristic(char_id).await?;
        let data = char.read().await?;
        Ok(data)
    }

    #[tracing::instrument(skip(self), fields(address = %self.address))]
    async fn try_connect(&self, retry: u8) -> anyhow::Result<()> {
        let mut count = retry;
        while count > 0 {
            if self.device.is_connected().await? {
                tracing::debug!("already connected");
                return Ok(());
            }
            match self.device.connect().await {
                Ok(_) => {
                    tracing::info!("device connected");
                    return Ok(());
                }
                Err(err) => {
                    tracing::warn!(message = "unable to connect", cause = %err);
                }
            }
            count -= 1;
        }
        Err(anyhow::anyhow!("unable to connect..."))
    }

    #[tracing::instrument(skip(self), fields(address = %self.address))]
    async fn read_system(&self) -> anyhow::Result<System> {
        let data = self.read(CHARACTERISTIC_FIRMWARE_ID).await?;
        Ok(System::from(data))
    }

    #[tracing::instrument(skip(self), fields(address = %self.address))]
    async fn read_realtime_values(&self) -> anyhow::Result<RealtimeEntry> {
        self.set_realtime_data_mode(true).await?;

        let data = self.read(CHARACTERISTIC_DATA_ID).await?;
        Ok(RealtimeEntry::from(data))
    }

    async fn set_realtime_data_mode(&self, enabled: bool) -> anyhow::Result<()> {
        self.set_device_mode(if enabled {
            &CMD_REALTIME_ENABLE
        } else {
            &REALTIME_DISABLE
        })
        .await
    }

    async fn set_device_mode(&self, payload: &[u8]) -> anyhow::Result<()> {
        let char = self.characteristic(CHARACTERISTIC_MODE_ID).await?;
        char.write_ext(
            payload,
            &CharacteristicWriteRequest {
                op_type: WriteOp::Request,
                ..Default::default()
            },
        )
        .await?;
        let data = char.read().await?;
        if !data.eq(payload) {
            return Err(anyhow::anyhow!("failed to write device mode"));
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(address = %self.address))]
    async fn try_disconnect(&self, retry: u8) -> anyhow::Result<()> {
        let mut count = retry;
        while count > 0 {
            if !self.device.is_connected().await? {
                tracing::debug!("already disconnected");
                return Ok(());
            }
            match self.device.disconnect().await {
                Ok(_) => {
                    tracing::info!("device disconnected");
                    return Ok(());
                }
                Err(err) => {
                    tracing::warn!(message = "unable to disconnect", cause = %err);
                }
            }
            count -= 1;
        }
        Err(anyhow::anyhow!("unable to disconnect..."))
    }
}

pub async fn handle(adapter: Adapter, addr: Address) -> anyhow::Result<()> {
    let device = adapter.device(addr)?;
    let miflora = Miflora {
        address: addr,
        device,
    };
    miflora.try_connect(5).await?;
    println!("info:   {:?}", miflora.read_system().await?);
    println!("values: {:?}", miflora.read_realtime_values().await?);
    miflora.try_disconnect(5).await?;
    Ok(())
}
