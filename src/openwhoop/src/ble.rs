use futures::stream::BoxStream;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BleNotification {
    pub uuid: Uuid,
    pub value: Vec<u8>,
}

pub type BleNotificationStream = BoxStream<'static, BleNotification>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BleWriteType {
    WithResponse,
    WithoutResponse,
}

#[allow(async_fn_in_trait)]
pub trait WhoopBleTransport {
    async fn connect(&self) -> anyhow::Result<()>;

    async fn disconnect(&self) -> anyhow::Result<()>;

    async fn is_connected(&self) -> anyhow::Result<bool>;

    async fn subscribe(&self, service: Uuid, characteristic: Uuid) -> anyhow::Result<()>;

    async fn read(&self, service: Uuid, characteristic: Uuid) -> anyhow::Result<Vec<u8>>;

    async fn write(
        &self,
        service: Uuid,
        characteristic: Uuid,
        data: &[u8],
        write_type: BleWriteType,
    ) -> anyhow::Result<()>;

    async fn notifications(&self) -> anyhow::Result<BleNotificationStream>;
}

pub mod btleplug_backend {
    use super::{BleNotification, BleNotificationStream, BleWriteType, WhoopBleTransport};
    use btleplug::{
        api::{
            Central, CharPropFlags, Characteristic, Peripheral as _, ValueNotification, WriteType,
        },
        platform::{Adapter, Peripheral},
    };
    use futures::StreamExt;
    use std::collections::BTreeSet;
    use uuid::Uuid;

    pub struct BtleplugTransport {
        peripheral: Peripheral,
        adapter: Adapter,
    }

    impl BtleplugTransport {
        pub fn new(peripheral: Peripheral, adapter: Adapter) -> Self {
            Self {
                peripheral,
                adapter,
            }
        }

        pub fn peripheral(&self) -> &Peripheral {
            &self.peripheral
        }

        pub fn adapter(&self) -> &Adapter {
            &self.adapter
        }

        fn create_char(service: Uuid, characteristic: Uuid) -> Characteristic {
            Characteristic {
                uuid: characteristic,
                service_uuid: service,
                properties: CharPropFlags::empty(),
                descriptors: BTreeSet::new(),
            }
        }
    }

    impl From<ValueNotification> for BleNotification {
        fn from(notification: ValueNotification) -> Self {
            Self {
                uuid: notification.uuid,
                value: notification.value,
            }
        }
    }

    impl From<BleWriteType> for WriteType {
        fn from(write_type: BleWriteType) -> Self {
            match write_type {
                BleWriteType::WithResponse => WriteType::WithResponse,
                BleWriteType::WithoutResponse => WriteType::WithoutResponse,
            }
        }
    }

    impl WhoopBleTransport for BtleplugTransport {
        async fn connect(&self) -> anyhow::Result<()> {
            self.peripheral.connect().await?;
            let _ = self.adapter.stop_scan().await;
            self.peripheral.discover_services().await?;
            Ok(())
        }

        async fn disconnect(&self) -> anyhow::Result<()> {
            if self.peripheral.is_connected().await? {
                self.peripheral.disconnect().await?;
            }
            Ok(())
        }

        async fn is_connected(&self) -> anyhow::Result<bool> {
            Ok(self.peripheral.is_connected().await?)
        }

        async fn subscribe(&self, service: Uuid, characteristic: Uuid) -> anyhow::Result<()> {
            self.peripheral
                .subscribe(&Self::create_char(service, characteristic))
                .await?;
            Ok(())
        }

        async fn read(&self, service: Uuid, characteristic: Uuid) -> anyhow::Result<Vec<u8>> {
            Ok(self
                .peripheral
                .read(&Self::create_char(service, characteristic))
                .await?)
        }

        async fn write(
            &self,
            service: Uuid,
            characteristic: Uuid,
            data: &[u8],
            write_type: BleWriteType,
        ) -> anyhow::Result<()> {
            self.peripheral
                .write(
                    &Self::create_char(service, characteristic),
                    data,
                    write_type.into(),
                )
                .await?;
            Ok(())
        }

        async fn notifications(&self) -> anyhow::Result<BleNotificationStream> {
            Ok(self
                .peripheral
                .notifications()
                .await?
                .map(Into::into)
                .boxed())
        }
    }
}

#[cfg(feature = "tauri-blec")]
pub mod tauri_blec {
    use super::{BleNotification, BleNotificationStream, BleWriteType, WhoopBleTransport};
    use anyhow::anyhow;
    use futures::{StreamExt, channel::mpsc};
    use openwhoop_codec::constants::{ALL_WHOOP_SERVICES, WhoopGeneration};
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tauri_plugin_blec::{
        Handler, OnDisconnectHandler,
        models::{BleDevice, ScanFilter, WriteType},
    };
    use tokio::time::{Instant, timeout};
    use uuid::Uuid;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct TauriBlecDevice {
        pub address: String,
        pub name: String,
        pub rssi: Option<i16>,
        pub generation: WhoopGeneration,
    }

    impl TauriBlecDevice {
        fn from_ble_device(device: BleDevice) -> Option<Self> {
            let generation = device.services.iter().find_map(|service| {
                if ALL_WHOOP_SERVICES.contains(service) {
                    WhoopGeneration::from_service(*service)
                } else {
                    None
                }
            })?;

            Some(Self {
                address: device.address,
                name: device.name,
                rssi: device.rssi,
                generation,
            })
        }
    }

    pub async fn scan_tauri_blec_devices(
        handler: &'static Handler,
        timeout_duration: Duration,
        allow_ibeacons: bool,
    ) -> anyhow::Result<Vec<TauriBlecDevice>> {
        let scan_duration = timeout_duration.max(Duration::from_millis(200));
        let timeout_ms = u64::try_from(scan_duration.as_millis()).unwrap_or(u64::MAX);
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);

        handler
            .discover(
                Some(tx),
                timeout_ms,
                ScanFilter::AnyService(ALL_WHOOP_SERVICES.to_vec()),
                allow_ibeacons,
            )
            .await?;

        let deadline = Instant::now() + scan_duration;
        let mut devices = BTreeMap::new();

        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }

            match timeout(deadline.saturating_duration_since(now), rx.recv()).await {
                Ok(Some(batch)) => {
                    for device in batch {
                        if let Some(device) = TauriBlecDevice::from_ble_device(device) {
                            devices.insert(device.address.clone(), device);
                        }
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }

        let _ = handler.stop_scan().await;
        Ok(devices.into_values().collect())
    }

    pub struct TauriBlecTransport {
        handler: &'static Handler,
        address: Option<String>,
        allow_ibeacons: bool,
        notification_sinks: Arc<Mutex<Vec<mpsc::UnboundedSender<BleNotification>>>>,
    }

    impl TauriBlecTransport {
        pub fn new(
            handler: &'static Handler,
            address: impl Into<String>,
            allow_ibeacons: bool,
        ) -> Self {
            Self {
                handler,
                address: Some(address.into()),
                allow_ibeacons,
                notification_sinks: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn connected(handler: &'static Handler) -> Self {
            Self {
                handler,
                address: None,
                allow_ibeacons: false,
                notification_sinks: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn handler(&self) -> &'static Handler {
            self.handler
        }
    }

    impl From<BleWriteType> for WriteType {
        fn from(write_type: BleWriteType) -> Self {
            match write_type {
                BleWriteType::WithResponse => WriteType::WithResponse,
                BleWriteType::WithoutResponse => WriteType::WithoutResponse,
            }
        }
    }

    impl WhoopBleTransport for TauriBlecTransport {
        async fn connect(&self) -> anyhow::Result<()> {
            if let Some(address) = self.address.as_deref() {
                self.handler
                    .connect(address, OnDisconnectHandler::None, self.allow_ibeacons)
                    .await?;
                return Ok(());
            }

            if self.handler.is_connected() {
                Ok(())
            } else {
                Err(anyhow!(
                    "tauri-plugin-blec handler is not connected; use TauriBlecTransport::new with an address or connect before constructing with connected()"
                ))
            }
        }

        async fn disconnect(&self) -> anyhow::Result<()> {
            self.handler.disconnect().await?;
            self.notification_sinks
                .lock()
                .map_err(|_| anyhow!("tauri blec notification sink mutex poisoned"))?
                .clear();
            Ok(())
        }

        async fn is_connected(&self) -> anyhow::Result<bool> {
            Ok(self.handler.is_connected())
        }

        async fn subscribe(&self, service: Uuid, characteristic: Uuid) -> anyhow::Result<()> {
            let notification_sinks = self.notification_sinks.clone();
            self.handler
                .subscribe(characteristic, Some(service), move |value: Vec<u8>| {
                    let notification = BleNotification {
                        uuid: characteristic,
                        value,
                    };
                    if let Ok(mut sinks) = notification_sinks.lock() {
                        sinks.retain(|sink| sink.unbounded_send(notification.clone()).is_ok());
                    }
                })
                .await?;
            Ok(())
        }

        async fn read(&self, service: Uuid, characteristic: Uuid) -> anyhow::Result<Vec<u8>> {
            Ok(self
                .handler
                .recv_data(characteristic, Some(service))
                .await?)
        }

        async fn write(
            &self,
            service: Uuid,
            characteristic: Uuid,
            data: &[u8],
            write_type: BleWriteType,
        ) -> anyhow::Result<()> {
            self.handler
                .send_data(characteristic, Some(service), data, write_type.into())
                .await?;
            Ok(())
        }

        async fn notifications(&self) -> anyhow::Result<BleNotificationStream> {
            let (tx, rx) = mpsc::unbounded();
            self.notification_sinks
                .lock()
                .map_err(|_| anyhow!("tauri blec notification sink mutex poisoned"))?
                .push(tx);
            Ok(rx.boxed())
        }
    }
}
