use std::borrow::Cow;
use std::time::{SystemTime, UNIX_EPOCH};

use bluer::gatt::remote::{Characteristic, CharacteristicWriteRequest};
use bluer::gatt::WriteOp;
use bluer::{Adapter, Address, Device};

/// These are the services/characteristics available on a miflora
/// service=58 characteristic=64
/// service=58 characteristic=59
/// service=58 characteristic=61
/// service=49 characteristic=55
/// service=49 characteristic=52
/// service=49 characteristic=50
/// service=12 characteristic=13
/// service=35 characteristic=38
/// service=35 characteristic=42
/// service=35 characteristic=40
/// service=35 characteristic=36
/// service=35 characteristic=44
/// service=35 characteristic=46
/// service=16 characteristic=28
/// service=16 characteristic=20
/// service=16 characteristic=26
/// service=16 characteristic=17
/// service=16 characteristic=32
/// service=16 characteristic=24
/// service=16 characteristic=22
/// service=16 characteristic=30

const DEVICE_UUID_PREFIX: u32 = 0xfe95;
const SERVICE_DATA_ID: u16 = 49;
const CHARACTERISTIC_MODE_ID: u16 = 50;
const CHARACTERISTIC_DATA_ID: u16 = 52;
const CHARACTERISTIC_FIRMWARE_ID: u16 = 0x37;

const SERVICE_HISTORY_ID: u16 = 58;
const CHARACTERISTIC_HISTORY_CTRL_ID: u16 = 61; // 0x3d; // 0x3e
const CHARACTERISTIC_HISTORY_READ_ID: u16 = 59; // 0x3b; // 0x3c
const CHARACTERISTIC_HISTORY_TIME_ID: u16 = 64;

// const CMD_BLINK_LED: [u8; 2] = [0xfd, 0xff];
const CMD_HISTORY_READ_INIT: [u8; 3] = [0xa0, 0x00, 0x00];
const CMD_HISTORY_READ_SUCCESS: [u8; 3] = [0xa2, 0x00, 0x00];
// const CMD_HISTORY_READ_FAILED: [u8; 3] = [0xa3, 0x00, 0x00];
const CMD_REALTIME_DISABLE: [u8; 2] = [0xc0, 0x1f];
const CMD_REALTIME_ENABLE: [u8; 2] = [0xa0, 0x1f];

const WRITE_OPTS: CharacteristicWriteRequest = CharacteristicWriteRequest {
    offset: 0,
    op_type: WriteOp::Request,
    prepare_authorize: false,
    _non_exhaustive: (),
};

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("went back in time")
        .as_secs_f64()
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("unable to find device with address {address}")]
    DeviceNotFound {
        address: Address,
        #[source]
        cause: bluer::Error,
    },
    #[error("unable to find service {service_id}")]
    ServiceNotFound {
        service_id: u16,
        #[source]
        cause: bluer::Error,
    },
    #[error("unable to find characteristic {characteristic_id} for service {service_id}")]
    CharacteristicNotFound {
        characteristic_id: u16,
        service_id: u16,
        #[source]
        cause: bluer::Error,
    },
    #[error("unable to read from service {service_id} and characteristic {characteristic_id}")]
    UnableToRead {
        characteristic_id: u16,
        service_id: u16,
        #[source]
        cause: bluer::Error,
    },
    #[error("unable to write to service {service_id} and characteristic {characteristic_id}")]
    UnableToWrite {
        characteristic_id: u16,
        service_id: u16,
        #[source]
        cause: bluer::Error,
    },
    #[error("the payload was not correctly written")]
    InvalidWrittenValue {
        characteristic_id: u16,
        service_id: u16,
    },
    #[error("unable to execute command with bluer")]
    CommandFailed {
        #[source]
        cause: bluer::Error,
    },
    #[error("too many retries")]
    TooManyRetries {
        retries: u8,
        #[source]
        cause: bluer::Error,
    },
    #[error("no service data provided")]
    NoServiceData,
    #[error("the provided device is not supported")]
    DeviceNotSupported,
}

#[derive(Clone)]
pub struct System {
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
pub struct RealtimeEntry {
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

/// Represents a historical entry of sensor values by parsing the byte array returned by the device.
///
/// The sensor returns 16 bytes in total.
/// It's unclear what the meaning of these bytes is beyond what is decoded in this method.
///
/// Semantics of the data (in little endian encoding):
/// bytes   0-3: timestamp, seconds since boot
/// bytes   4-5: temperature in 0.1 °C
/// byte      6: unknown
/// bytes   7-9: brightness in lux
/// byte     10: unknown
/// byte     11: moisture in %
/// bytes 12-13: conductivity in µS/cm
/// bytes 14-15: unknown
///
/// (source https://github.com/vrachieru/xiaomi-flower-care-api/blob/master/flowercare/reader.py#L160)
#[derive(Clone)]
pub struct HistoricalEntry {
    epoch_time: u64,
    inner: Vec<u8>,
}

impl HistoricalEntry {
    fn new(inner: Vec<u8>, epoch_time: u64) -> Self {
        Self { epoch_time, inner }
    }

    pub fn timestamp(&self) -> u64 {
        let offset =
            u32::from_le_bytes([self.inner[0], self.inner[1], self.inner[2], self.inner[3]]);
        self.epoch_time + offset as u64
    }

    pub fn temperature(&self) -> u16 {
        u16::from_le_bytes([self.inner[4], self.inner[5]])
    }

    pub fn brightness(&self) -> u32 {
        u32::from_le_bytes([self.inner[7], self.inner[8], self.inner[9], 0])
    }

    pub fn moisture(&self) -> u8 {
        self.inner[11]
    }

    pub fn conductivity(&self) -> u16 {
        u16::from_le_bytes([self.inner[12], self.inner[13]])
    }
}

impl std::fmt::Debug for HistoricalEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(stringify!(HistoricalEntry))
            .field("timestamp", &self.timestamp())
            .field("temperature", &self.temperature())
            .field("brightness", &self.brightness())
            .field("moisture", &self.moisture())
            .field("conductivity", &self.conductivity())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct Miflora {
    device: Device,
}

impl From<Device> for Miflora {
    fn from(device: Device) -> Self {
        Self { device }
    }
}

pub async fn is_miflora_device(device: &Device) -> Result<bool, Error> {
    let service_data = device
        .service_data()
        .await
        .map_err(|err| Error::CommandFailed { cause: err })?;
    let service_data = service_data.ok_or(Error::NoServiceData)?;
    Ok(service_data.iter().any(|(uuid, _data)| {
        let (id, _, _, _) = uuid.as_fields();
        id == DEVICE_UUID_PREFIX
    }))
}

impl Miflora {
    pub async fn try_from_adapter(adapter: &Adapter, address: Address) -> Result<Self, Error> {
        let device = adapter
            .device(address)
            .map_err(|err| Error::DeviceNotFound {
                address,
                cause: err,
            })?;
        Self::try_from_device(device).await
    }

    pub async fn try_from_device(device: Device) -> Result<Self, Error> {
        if is_miflora_device(&device).await? {
            Ok(Self { device })
        } else {
            Err(Error::DeviceNotSupported)
        }
    }

    async fn characteristic(&self, service_id: u16, char_id: u16) -> Result<Characteristic, Error> {
        let service =
            self.device
                .service(service_id)
                .await
                .map_err(|err| Error::ServiceNotFound {
                    service_id,
                    cause: err,
                })?;
        let char =
            service
                .characteristic(char_id)
                .await
                .map_err(|err| Error::CharacteristicNotFound {
                    characteristic_id: char_id,
                    service_id,
                    cause: err,
                })?;
        Ok(char)
    }

    async fn read(&self, service_id: u16, char_id: u16) -> Result<Vec<u8>, Error> {
        let char = self.characteristic(service_id, char_id).await?;
        tracing::trace!(
            message = "reading",
            service = service_id,
            characteristic = char_id
        );
        char.read().await.map_err(|err| Error::UnableToRead {
            characteristic_id: char_id,
            service_id,
            cause: err,
        })
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn is_connected(&self) -> Result<bool, Error> {
        self.device
            .is_connected()
            .await
            .map_err(|err| Error::CommandFailed { cause: err })
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn connect(&self) -> Result<(), Error> {
        self.device
            .connect()
            .await
            .map_err(|err| Error::CommandFailed { cause: err })
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn try_connect(&self, retry: u8) -> Result<(), Error> {
        let mut count = 0;
        loop {
            if self.is_connected().await? {
                tracing::debug!("already connected");
                return Ok(());
            }
            match self.device.connect().await {
                Ok(_) => {
                    tracing::info!("device connected");
                    return Ok(());
                }
                Err(err) => {
                    count += 1;
                    tracing::warn!(message = "unable to connect", tries = count, cause = %err);
                    if count > retry {
                        return Err(Error::TooManyRetries {
                            retries: count,
                            cause: err,
                        });
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn disconnect(&self) -> Result<(), Error> {
        self.device
            .disconnect()
            .await
            .map_err(|err| Error::CommandFailed { cause: err })
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn try_disconnect(&self, retry: u8) -> Result<(), Error> {
        let mut count = 0;
        loop {
            if !self.is_connected().await? {
                tracing::debug!("already disconnected");
                return Ok(());
            }
            match self.device.disconnect().await {
                Ok(_) => {
                    tracing::info!("device disconnected");
                    return Ok(());
                }
                Err(err) => {
                    count += 1;
                    tracing::warn!(message = "unable to disconnect", tries = count, cause = %err);
                    if count > retry {
                        return Err(Error::TooManyRetries {
                            retries: count,
                            cause: err,
                        });
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn read_system(&self) -> Result<System, Error> {
        let data = self
            .read(SERVICE_DATA_ID, CHARACTERISTIC_FIRMWARE_ID)
            .await?;
        Ok(System::from(data))
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn read_realtime_values(&self) -> Result<RealtimeEntry, Error> {
        self.set_realtime_data_mode(true).await?;

        let data = self.read(SERVICE_DATA_ID, CHARACTERISTIC_DATA_ID).await?;
        Ok(RealtimeEntry::from(data))
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn read_epoch_time(&self) -> Result<u64, Error> {
        let start = now();
        let char = self
            .characteristic(SERVICE_HISTORY_ID, CHARACTERISTIC_HISTORY_TIME_ID)
            .await?;
        tracing::trace!(
            message = "reading",
            service = SERVICE_HISTORY_ID,
            characteristic = CHARACTERISTIC_HISTORY_TIME_ID
        );
        let data = char.read().await.map_err(|err| Error::UnableToWrite {
            characteristic_id: CHARACTERISTIC_HISTORY_TIME_ID,
            service_id: SERVICE_HISTORY_ID,
            cause: err,
        })?;
        let wall_time = (now() + start) / 2.0;
        let epoch_offset = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let epoch_time = (wall_time as u64) - (epoch_offset as u64);
        Ok(epoch_time)
    }

    fn historical_entry_address(&self, index: u16) -> [u8; 3] {
        let bytes = u16::to_le_bytes(index);
        [0xa1, bytes[0], bytes[1]]
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn read_historical_values(&self) -> Result<Vec<HistoricalEntry>, Error> {
        let ctrl_char = self
            .characteristic(SERVICE_HISTORY_ID, CHARACTERISTIC_HISTORY_CTRL_ID)
            .await?;
        tracing::trace!(
            message = "writing",
            service = SERVICE_HISTORY_ID,
            characteristic = CHARACTERISTIC_HISTORY_CTRL_ID
        );
        ctrl_char
            .write_ext(&CMD_HISTORY_READ_INIT, &WRITE_OPTS)
            .await
            .map_err(|err| Error::UnableToWrite {
                characteristic_id: CHARACTERISTIC_HISTORY_CTRL_ID,
                service_id: SERVICE_HISTORY_ID,
                cause: err,
            })?;
        //
        let char = self
            .characteristic(SERVICE_HISTORY_ID, CHARACTERISTIC_HISTORY_READ_ID)
            .await?;
        tracing::trace!(
            message = "reading",
            service = SERVICE_HISTORY_ID,
            characteristic = CHARACTERISTIC_HISTORY_READ_ID
        );
        let raw_history_data = char.read().await.map_err(|err| Error::UnableToRead {
            characteristic_id: CHARACTERISTIC_HISTORY_READ_ID,
            service_id: SERVICE_HISTORY_ID,
            cause: err,
        })?;
        let history_length = u16::from_le_bytes([raw_history_data[0], raw_history_data[1]]);
        //
        let mut result = Vec::with_capacity(history_length as usize);
        if history_length > 0 {
            let epoch_time = self.read_epoch_time().await?;
            let read_char = self
                .characteristic(SERVICE_HISTORY_ID, CHARACTERISTIC_HISTORY_READ_ID)
                .await?;
            for i in 0..history_length {
                tracing::debug!("loading entry {i}");
                let payload = self.historical_entry_address(i);
                tracing::trace!(
                    message = "writing",
                    service = SERVICE_HISTORY_ID,
                    characteristic = CHARACTERISTIC_HISTORY_CTRL_ID
                );
                ctrl_char
                    .write_ext(&payload, &WRITE_OPTS)
                    .await
                    .map_err(|err| Error::UnableToWrite {
                        characteristic_id: CHARACTERISTIC_HISTORY_CTRL_ID,
                        service_id: SERVICE_HISTORY_ID,
                        cause: err,
                    })?;
                tracing::trace!(
                    message = "reading",
                    service = SERVICE_HISTORY_ID,
                    characteristic = CHARACTERISTIC_HISTORY_READ_ID
                );
                let data = read_char.read().await.map_err(|err| Error::UnableToRead {
                    characteristic_id: CHARACTERISTIC_HISTORY_READ_ID,
                    service_id: SERVICE_HISTORY_ID,
                    cause: err,
                })?;
                result.push(HistoricalEntry::new(data, epoch_time));
            }
        }
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(address = %self.device.address()))]
    pub async fn clear_historical_entries(&self) -> Result<(), Error> {
        let ctrl_char = self
            .characteristic(SERVICE_HISTORY_ID, CHARACTERISTIC_HISTORY_CTRL_ID)
            .await?;
        tracing::trace!(
            message = "writing",
            service = SERVICE_HISTORY_ID,
            characteristic = CHARACTERISTIC_HISTORY_CTRL_ID
        );
        ctrl_char
            .write_ext(&CMD_HISTORY_READ_SUCCESS, &WRITE_OPTS)
            .await
            .map_err(|err| Error::UnableToRead {
                characteristic_id: CHARACTERISTIC_HISTORY_CTRL_ID,
                service_id: SERVICE_HISTORY_ID,
                cause: err,
            })?;
        Ok(())
    }

    async fn set_realtime_data_mode(&self, enabled: bool) -> Result<(), Error> {
        self.set_device_mode(if enabled {
            &CMD_REALTIME_ENABLE
        } else {
            &CMD_REALTIME_DISABLE
        })
        .await
    }

    async fn set_device_mode(&self, payload: &[u8]) -> Result<(), Error> {
        let char = self
            .characteristic(SERVICE_DATA_ID, CHARACTERISTIC_MODE_ID)
            .await?;
        tracing::trace!(
            message = "writing",
            service = SERVICE_DATA_ID,
            characteristic = CHARACTERISTIC_MODE_ID
        );
        char.write_ext(payload, &WRITE_OPTS)
            .await
            .map_err(|err| Error::UnableToWrite {
                service_id: SERVICE_DATA_ID,
                characteristic_id: CHARACTERISTIC_MODE_ID,
                cause: err,
            })?;
        let data = char.read().await.map_err(|err| Error::UnableToRead {
            characteristic_id: CHARACTERISTIC_MODE_ID,
            service_id: SERVICE_DATA_ID,
            cause: err,
        })?;
        if !data.eq(payload) {
            return Err(Error::InvalidWrittenValue {
                characteristic_id: CHARACTERISTIC_MODE_ID,
                service_id: SERVICE_DATA_ID,
            });
        }
        Ok(())
    }
}
