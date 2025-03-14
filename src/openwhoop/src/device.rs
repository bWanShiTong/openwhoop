use std::{collections::BTreeSet, time::Duration};

use btleplug::{
    api::{CharPropFlags, Characteristic, Peripheral as _, WriteType},
    platform::Peripheral,
};
use futures::StreamExt;
use tokio::time::sleep;
use uuid::Uuid;
use whoop::{
    constants::{
        CMD_FROM_STRAP, CMD_TO_STRAP, DATA_FROM_STRAP, EVENTS_FROM_STRAP, MEMFAULT, WHOOP_SERVICE,
    },
    WhoopPacket,
};

use crate::{openwhoop::OpenWhoop, DatabaseHandler};

pub struct WhoopDevice {
    peripheral: Peripheral,
    whoop: OpenWhoop,
}

impl WhoopDevice {
    pub fn new(peripheral: Peripheral, db: DatabaseHandler) -> Self {
        Self {
            peripheral,
            whoop: OpenWhoop::new(db),
        }
    }

    pub async fn connect(&mut self) -> anyhow::Result<()> {
        self.peripheral.connect().await?;
        self.peripheral.discover_services().await?;
        Ok(())
    }
    pub async fn connect_and_initialize(&mut self) -> anyhow::Result<()> {
        self.peripheral.connect().await?;
        self.peripheral.discover_services().await?;
    
        // Send the "wake-up" or initialization writes (replace with actual UUID)
        self.send_handshake_requests().await?;
    
        Ok(())
    }
    
    pub async fn send_handshake_requests(&mut self) -> anyhow::Result<()> {
        let handshake_packets: Vec<Vec<u8>> = vec![
            vec![0x02, 0x08, 0x00, 0x09, 0x00, 0x05, 0x00, 0x04, 0x00, 0x12, 0x16, 0x00, 0x01, 0x00],
            vec![0x02, 0x08, 0x00, 0x09, 0x00, 0x05, 0x00, 0x04, 0x00, 0x12, 0x19, 0x00, 0x01, 0x00],
            vec![0x02, 0x08, 0x00, 0x09, 0x00, 0x05, 0x00, 0x04, 0x00, 0x12, 0x1c, 0x00, 0x01, 0x00],
            vec![0x02, 0x08, 0x00, 0x09, 0x00, 0x05, 0x00, 0x04, 0x00, 0x12, 0x13, 0x00, 0x01, 0x00],
        ];
    
        for packet in handshake_packets {
            self.peripheral
                .write(
                    &Self::create_char(CMD_TO_STRAP), // Use the correct characteristic
                    &packet,
                    WriteType::WithResponse,  // Make sure this is a "Write Request"
                )
                .await?;
        }
    
        Ok(())
    }
    
    pub async fn is_connected(&mut self) -> anyhow::Result<bool> {
        let is_connected = self.peripheral.is_connected().await?;
        Ok(is_connected)
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

        self.send_command(WhoopPacket::enter_high_freq_sync())
            .await?;

        // self.send_command(WhoopPacket::hello_harvard()).await?;
        // self.send_command(WhoopPacket::set_time()).await?;
        // self.send_command(WhoopPacket::get_name()).await?;

        Ok(())
    }


    pub async fn init(&mut self) -> anyhow::Result<()> {
        self.subscribe(DATA_FROM_STRAP).await?;
        self.subscribe(CMD_FROM_STRAP).await?;
        self.subscribe(EVENTS_FROM_STRAP).await?;
        self.subscribe(MEMFAULT).await?;

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
                    let packet = self.whoop.store_packet(notification).await?;
                    if let Some(packet) = self.whoop.handle_packet(packet).await?{
                        self.send_command(packet).await?;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn get_name(&mut self) -> anyhow::Result<()> {
        println!("Subscribing to notifications...");
        let mut notifications = self.peripheral.notifications().await?;
    
        // Debug: Print all available characteristics
        //let characteristics = self.peripheral.characteristics();
        //println!("Available Characteristics: {:?}", characteristics);

        // Debug: Print the raw command packet
        let command = WhoopPacket::get_name();
        println!("Sending GetName command: {:?}", command.framed_packet());

        // Debug: Print the raw hex packet
        println!("Sending GetName command in hex: {:?}", hex::encode(&command.framed_packet()));

        self.send_command(command).await?;

        // Debug: Reinitialize after sending the command
        self.initialize().await?;

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
                    // Print all notifications
                    println!("Notification uuid: {:?}", notification.uuid);
                    // Print hex representation of the notification value
                    println!("Hex data: {}", notification.value.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "));
                    // decode received hex notification.value
                    let packet = WhoopPacket::from_data(notification.value);
                    // Print decoded packet
                    println!("Packet: {:?}", packet);
                    }
                }
            }

        Ok(())
    }

    pub async fn reboot(&mut self) -> anyhow::Result<()> {
        self.send_command(WhoopPacket::reboot()).await?;

        Ok(())
    }

    async fn on_sleep(&mut self) -> anyhow::Result<bool> {
        let is_connected = self.peripheral.is_connected().await?;
        Ok(!is_connected)
    }
}
