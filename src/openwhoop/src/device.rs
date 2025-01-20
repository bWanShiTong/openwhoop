use std::{collections::BTreeSet, time::Duration};

use btleplug::{
    api::{CharPropFlags, Characteristic, Peripheral as _, ValueNotification, WriteType},
    platform::Peripheral,
};
use futures::StreamExt;
use tokio::time::sleep;
use uuid::Uuid;
use whoop::{
    constants::{
        MetadataType, CMD_FROM_STRAP, CMD_TO_STRAP, DATA_FROM_STRAP, EVENTS_FROM_STRAP, MEMFAULT,
        WHOOP_SERVICE,
    },
    WhoopData, WhoopPacket,
};

use crate::DatabaseHandler;

pub struct Whoop {
    peripheral: Peripheral,
    db: DatabaseHandler,
}

impl Whoop {
    pub fn new(peripheral: Peripheral, db: DatabaseHandler) -> Self {
        Self { peripheral, db }
    }

    pub async fn connect(&mut self) -> anyhow::Result<()> {
        self.peripheral.connect().await?;
        self.peripheral.discover_services().await?;
        Ok(())
    }

    fn create_char(characteristic: Uuid) -> Characteristic {
        Characteristic {
            uuid: characteristic,
            service_uuid: WHOOP_SERVICE,
            properties: CharPropFlags::empty(),
            descriptors: BTreeSet::new(),
        }
    }

    async fn subscribe(&self, char: Uuid) -> anyhow::Result<()> {
        self.peripheral.subscribe(&Self::create_char(char)).await?;
        Ok(())
    }

    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        self.subscribe(DATA_FROM_STRAP).await?;
        self.subscribe(CMD_FROM_STRAP).await?;
        self.subscribe(EVENTS_FROM_STRAP).await?;
        self.subscribe(MEMFAULT).await?;

        // self.send_command(WhoopPacket::hello_harvard()).await?;
        // self.send_command(WhoopPacket::set_time()).await?;
        // self.send_command(WhoopPacket::get_name()).await?;

        Ok(())
    }

    pub async fn send_command(&mut self, packet: WhoopPacket) -> anyhow::Result<()> {
        let packet = packet.framed_packet();
        self.peripheral
            .write(
                &Self::create_char(CMD_TO_STRAP),
                &packet,
                WriteType::WithoutResponse,
            )
            .await?;
        Ok(())
    }

    pub async fn sync_history(&mut self) -> anyhow::Result<()> {
        let mut notifications = self.peripheral.notifications().await?;
        self.send_command(WhoopPacket::history_start()).await?;

        loop {
            let notification = notifications.next();
            let sleep = sleep(Duration::from_secs(10));

            tokio::select! {
                _ = sleep => {
                    if self.on_sleep().await?{
                        error!("Whoop disconnected");
                        break;
                    }
                },
                Some(notification) = notification => {
                    if self.handle_notification(notification).await?{
                        break
                    }
                }
            }
        }

        Ok(())
    }

    async fn on_sleep(&mut self) -> anyhow::Result<bool> {
        let is_connected = self.peripheral.is_connected().await?;
        Ok(!is_connected)
    }

    async fn handle_notification(
        &mut self,
        notification: ValueNotification,
    ) -> anyhow::Result<bool> {
        let packet = self
            .db
            .create_packet(notification.uuid, notification.value)
            .await?;

        match packet.uuid {
            DATA_FROM_STRAP => {
                let Ok(packet) = WhoopPacket::from_data(packet.bytes) else {
                    return Ok(false);
                };

                let Ok(data) = WhoopData::from_packet(packet) else {
                    return Ok(false);
                };

                match data {
                    WhoopData::HistoryReading { unix, bpm, rr } => {
                        self.db.create_reading(unix, bpm, rr).await?;
                    }
                    WhoopData::HistoryMetadata { data, cmd, .. } => match cmd {
                        MetadataType::HistoryComplete => return Ok(true),
                        MetadataType::HistoryStart => {}
                        MetadataType::HistoryEnd => {
                            let packet = WhoopPacket::history_end(data);
                            self.send_command(packet).await?;
                        }
                    },
                    WhoopData::ConsoleLog { log, .. } => {
                        trace!(target: "ConsoleLog", "{}", log);
                    }
                    WhoopData::RunAlarm { .. } => {}
                    WhoopData::Event { .. } => {}
                    WhoopData::UnknownEvent { .. } => {}
                }
            }
            _ => {
                // todo!()
            }
        }

        Ok(false)
    }
}
