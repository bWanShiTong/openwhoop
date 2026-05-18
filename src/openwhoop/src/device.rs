use anyhow::anyhow;
use chrono::{Local, Utc};
use futures::StreamExt;
use openwhoop_algos::StressCalculator;
use openwhoop_codec::{ParsedHistoryReading, WhoopData, WhoopPacket, constants::WhoopGeneration};
use openwhoop_entities::packets::Model;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::time::{sleep, timeout};
use uuid::{Uuid, uuid};

use crate::{
    ble::{BleNotification, BleWriteType, WhoopBleTransport, btleplug_backend::BtleplugTransport},
    db::DatabaseHandler,
    openwhoop::OpenWhoop,
};

#[path = "sync/gen5.rs"]
mod gen5_sync;

use self::gen5_sync::Gen5HistorySync;

#[derive(Debug, Clone, Copy)]
pub struct HistorySyncConfig {
    pub overall_timeout: Option<Duration>,
    pub idle_timeout: Duration,
    pub exit_on_failure: bool,
}

impl Default for HistorySyncConfig {
    fn default() -> Self {
        Self {
            overall_timeout: None,
            idle_timeout: Duration::from_secs(20),
            exit_on_failure: false,
        }
    }
}

impl HistorySyncConfig {
    pub fn from_secs(overall_timeout_secs: u64, idle_timeout_secs: u64) -> Self {
        Self {
            overall_timeout: (overall_timeout_secs > 0)
                .then(|| Duration::from_secs(overall_timeout_secs)),
            idle_timeout: Duration::from_secs(idle_timeout_secs.max(1)),
            exit_on_failure: false,
        }
    }

    pub fn with_exit_on_failure(mut self, exit_on_failure: bool) -> Self {
        self.exit_on_failure = exit_on_failure;
        self
    }
}

const REALTIME_STREAM_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_REALTIME_STRESS_SAMPLES: usize = 8;
const BATTERY_SERVICE_UUID: Uuid = uuid!("0000180f-0000-1000-8000-00805f9b34fb");
const BATTERY_LEVEL_CHARACTERISTIC_UUID: Uuid = uuid!("00002a19-0000-1000-8000-00805f9b34fb");

#[derive(Default)]
struct RealtimeStressWindow {
    readings: VecDeque<ParsedHistoryReading>,
}

impl RealtimeStressWindow {
    fn push(&mut self, unix: u32, bpm: u8) -> Option<f64> {
        if bpm == 0 {
            return None;
        }

        let time = chrono::DateTime::from_timestamp(i64::from(unix), 0)
            .map(|timestamp| timestamp.naive_utc())
            .unwrap_or_else(|| Utc::now().naive_utc());

        self.readings.push_back(ParsedHistoryReading {
            time,
            bpm,
            rr: Vec::new(),
            imu_data: None,
            gravity: None,
        });

        while self.readings.len() > StressCalculator::MIN_READING_PERIOD {
            self.readings.pop_front();
        }

        let min_samples = MIN_REALTIME_STRESS_SAMPLES.min(StressCalculator::MIN_READING_PERIOD);
        if self.readings.len() < min_samples {
            return None;
        }

        let readings = self.readings.make_contiguous();
        StressCalculator::calculate_stress_with_min_reading_period(readings, min_samples)
            .map(|stress| stress.score)
    }
}

fn format_realtime_time(unix: u32) -> String {
    chrono::DateTime::from_timestamp(i64::from(unix), 0)
        .map(|time| time.with_timezone(&Local).format("%H:%M:%S").to_string())
        .unwrap_or_else(|| unix.to_string())
}

pub type WhoopDevice = WhoopDeviceWith<BtleplugTransport>;

pub struct WhoopDeviceWith<T> {
    transport: T,
    whoop: OpenWhoop,
    debug_packets: bool,
    generation: WhoopGeneration,
    seq: u8,
}

impl WhoopDeviceWith<BtleplugTransport> {
    pub fn new(
        peripheral: btleplug::platform::Peripheral,
        adapter: btleplug::platform::Adapter,
        db: DatabaseHandler,
        debug_packets: bool,
        generation: WhoopGeneration,
    ) -> Self {
        Self::from_transport(
            BtleplugTransport::new(peripheral, adapter),
            db,
            debug_packets,
            generation,
        )
    }
}

impl<T> WhoopDeviceWith<T>
where
    T: WhoopBleTransport,
{
    pub fn from_transport(
        transport: T,
        db: DatabaseHandler,
        debug_packets: bool,
        generation: WhoopGeneration,
    ) -> Self {
        Self {
            transport,
            whoop: OpenWhoop::new(db, generation),
            debug_packets,
            generation,
            seq: 0,
        }
    }

    pub async fn connect(&mut self) -> anyhow::Result<()> {
        self.transport.connect().await?;
        self.whoop.packet = None;
        self.seq = 0;
        Ok(())
    }

    pub async fn disconnect(&mut self) -> anyhow::Result<()> {
        self.transport.disconnect().await?;
        self.whoop.packet = None;
        Ok(())
    }

    pub async fn is_connected(&mut self) -> anyhow::Result<bool> {
        self.transport.is_connected().await
    }

    async fn subscribe(&self, characteristic: uuid::Uuid) -> anyhow::Result<()> {
        self.transport
            .subscribe(self.generation.service(), characteristic)
            .await
    }

    pub async fn initialize(&mut self) -> anyhow::Result<()> {
        let generation = self.generation;
        self.subscribe(generation.data_from_strap()).await?;
        self.subscribe(generation.cmd_from_strap()).await?;
        self.subscribe(generation.events_from_strap()).await?;
        self.subscribe(generation.memfault()).await?;
        Ok(())
    }

    pub async fn send_command(&mut self, packet: WhoopPacket) -> anyhow::Result<()> {
        self.send_command_with_seq(packet).await.map(|_| ())
    }

    async fn send_command_with_seq(&mut self, packet: WhoopPacket) -> anyhow::Result<u8> {
        let seq = self.seq;
        let packet = packet.with_seq(seq);
        self.seq = self.seq.wrapping_add(1);
        let bytes = match self.generation {
            WhoopGeneration::Gen4 => packet.framed_packet()?,
            WhoopGeneration::Gen5 => packet.framed_packet_maverick()?,
            WhoopGeneration::Placeholder => {
                return Err(anyhow!(
                    "WhoopGeneration::Placeholder cannot be used for BLE command transport"
                ));
            }
        };
        self.transport
            .write(
                self.generation.service(),
                self.generation.cmd_to_strap(),
                &bytes,
                BleWriteType::WithoutResponse,
            )
            .await?;
        Ok(seq)
    }

    pub async fn sync_history(
        &mut self,
        should_exit: Arc<AtomicBool>,
        config: HistorySyncConfig,
    ) -> anyhow::Result<()> {
        match self.generation {
            WhoopGeneration::Gen4 => self.sync_history_gen4(should_exit).await,
            WhoopGeneration::Gen5 => self.sync_history_gen5(should_exit, config).await,
            WhoopGeneration::Placeholder => Err(anyhow!(
                "WhoopGeneration::Placeholder cannot be used for history sync"
            )),
        }
    }

    async fn sync_history_gen4(&mut self, should_exit: Arc<AtomicBool>) -> anyhow::Result<()> {
        let mut notifications = self.transport.notifications().await?;

        self.send_command(WhoopPacket::hello_harvard()).await?;
        self.send_command(WhoopPacket::set_time()?).await?;
        self.send_command(WhoopPacket::get_name()).await?;
        self.send_command(WhoopPacket::enter_high_freq_sync())
            .await?;
        self.send_command(WhoopPacket::history_start()).await?;

        loop {
            if should_exit.load(Ordering::SeqCst) {
                break;
            }
            tokio::select! {
                _ = sleep(Duration::from_secs(10)) => {
                    if self.on_sleep().await? {
                        error!("Whoop disconnected");
                    }
                    break;
                }
                Some(notification) = notifications.next() => {
                    self.handle_sync_notification(notification).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn sync_history_gen5(
        &mut self,
        should_exit: Arc<AtomicBool>,
        config: HistorySyncConfig,
    ) -> anyhow::Result<()> {
        const FULL_SESSION_RETRY_WAIT: Duration = Duration::from_secs(45);

        let mut reconnect_attempted = false;
        let mut initial_failure = None;

        loop {
            let notifications = self.transport.notifications().await?;
            match Gen5HistorySync::new(self, should_exit.clone(), notifications, config)
                .start()
                .await
            {
                Ok(()) => return Ok(()),
                Err(err)
                    if !reconnect_attempted
                        && config.exit_on_failure
                        && should_retry_gen5_history_after_full_reconnect(&err) =>
                {
                    reconnect_attempted = true;
                    initial_failure = Some(err.to_string());
                    warn!(
                        "Gen5 history sync failed in a stale-session pattern; forcing BLE disconnect/reconnect and retry once: {err}"
                    );
                    self.reconnect_for_gen5_history_retry(
                        should_exit.clone(),
                        FULL_SESSION_RETRY_WAIT,
                    )
                    .await
                    .map_err(|retry_err| {
                        let initial = initial_failure.as_deref().unwrap_or("unknown failure");
                        retry_err.context(format!(
                            "failed to reconnect strap for Gen5 history retry after initial failure: {initial}"
                        ))
                    })?;
                }
                Err(err) => {
                    if let Some(initial) = initial_failure.as_deref() {
                        return Err(err.context(format!(
                            "Gen5 history sync still failed after forced reconnect; initial failure: {initial}"
                        )));
                    }
                    return Err(err);
                }
            }
        }
    }

    async fn handle_sync_notification(
        &mut self,
        notification: BleNotification,
    ) -> anyhow::Result<()> {
        let packet = self.notification_to_model(notification).await?;
        if let Some(reply) = self.whoop.handle_packet(packet).await? {
            self.send_command(reply).await?;
        }
        Ok(())
    }

    async fn notification_to_model(&self, notification: BleNotification) -> anyhow::Result<Model> {
        match self.debug_packets {
            true => self.whoop.store_packet(notification).await,
            false => Ok(Model {
                id: 0,
                uuid: notification.uuid,
                generation: self.generation.to_string(),
                bytes: notification.value,
            }),
        }
    }

    async fn on_sleep(&mut self) -> anyhow::Result<bool> {
        let is_connected = self.transport.is_connected().await?;
        Ok(!is_connected)
    }

    async fn reconnect_for_gen5_history_retry(
        &mut self,
        should_exit: Arc<AtomicBool>,
        max_wait: Duration,
    ) -> anyhow::Result<()> {
        let disconnect_needed = self.transport.is_connected().await.unwrap_or(false);
        if disconnect_needed {
            if let Err(err) = self.disconnect().await {
                warn!("BLE disconnect before Gen5 history retry failed: {err}");
            }
        }

        let deadline = Instant::now() + max_wait;
        let mut last_err = None;

        while !should_exit.load(Ordering::SeqCst) {
            match self.connect().await {
                Ok(()) => match self.initialize().await {
                    Ok(()) => return Ok(()),
                    Err(err) => {
                        last_err = Some(err.context(
                            "connected to strap but failed to reinitialize BLE subscriptions",
                        ));
                        let _ = self.disconnect().await;
                    }
                },
                Err(err) => {
                    last_err = Some(err.context("failed to reconnect to strap"));
                }
            }

            if Instant::now() >= deadline {
                break;
            }

            sleep(Duration::from_secs(2)).await;
        }

        Err(last_err.unwrap_or_else(|| anyhow!("timed out waiting for strap to reconnect")))
    }

    /// Ring the device. Dispatches to the correct command for the generation:
    /// - Gen4: RunAlarm (cmd=68)
    /// - Maverick: RunHapticPatternMaverick / WSBLE_CMD_HAPTICS_RUN_NTF (cmd=19, revision=0x01)
    pub async fn ring_alarm(&mut self) -> anyhow::Result<()> {
        let packet = match self.generation {
            WhoopGeneration::Gen4 => WhoopPacket::run_alarm_now(),
            WhoopGeneration::Gen5 => WhoopPacket::run_haptic_pattern_gen5(),
            WhoopGeneration::Placeholder => {
                return Err(anyhow!("WhoopGeneration::Placeholder cannot ring a device"));
            }
        };
        self.send_command(packet).await
    }

    async fn stream_realtime<F>(
        &mut self,
        should_exit: Arc<AtomicBool>,
        mut on_hr: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(u32, u8) -> anyhow::Result<()>,
    {
        let generation = self.generation;
        self.subscribe(generation.data_from_strap()).await?;
        self.subscribe(generation.cmd_from_strap()).await?;

        let mut notifications = self.transport.notifications().await?;
        self.send_command(WhoopPacket::toggle_realtime_hr(true))
            .await?;

        let stream_result = async {
            loop {
                if should_exit.load(Ordering::SeqCst) {
                    break;
                }
                let notification = notifications.next();
                let sleep_ = sleep(REALTIME_STREAM_TIMEOUT);

                tokio::select! {
                    _ = sleep_ => {
                        warn!("Timed out waiting for realtime HR data");
                        break;
                    },
                    Some(notification) = notification => {
                        let bytes = notification.value;
                        let packet = match generation {
                            WhoopGeneration::Gen4 => WhoopPacket::from_data(bytes),
                            WhoopGeneration::Gen5 => WhoopPacket::from_data_maverick(bytes),
                            WhoopGeneration::Placeholder => {
                                return Err(anyhow!(
                                    "WhoopGeneration::Placeholder cannot parse realtime HR packets"
                                ));
                            }
                        };
                        let packet = match packet {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let decoded = match generation {
                            WhoopGeneration::Gen4 => WhoopData::from_packet_gen4(packet),
                            WhoopGeneration::Gen5 => WhoopData::from_packet_gen5(packet),
                            WhoopGeneration::Placeholder => {
                                return Err(anyhow!(
                                    "WhoopGeneration::Placeholder cannot decode realtime HR packets"
                                ));
                            }
                        };
                        match decoded {
                            Ok(WhoopData::RealtimeHr { unix, bpm }) => on_hr(unix, bpm)?,
                            Ok(WhoopData::Event { .. } | WhoopData::UnknownEvent { .. }) => {}
                            _ => {}
                        }
                    }
                }
            }
            Ok(())
        }
        .await;

        if let Ok(true) = self.transport.is_connected().await {
            if let Err(err) = self
                .send_command(WhoopPacket::toggle_realtime_hr(false))
                .await
            {
                if stream_result.is_ok() {
                    return Err(err);
                }
                warn!("Failed to disable realtime HR stream: {err}");
            }
        }

        stream_result
    }

    /// Stream realtime heart rate until Ctrl-C or timeout.
    pub async fn stream_hr(&mut self, should_exit: Arc<AtomicBool>) -> anyhow::Result<()> {
        self.stream_realtime(should_exit, |unix, bpm| {
            println!("{} HR: {} bpm", format_realtime_time(unix), bpm);
            Ok(())
        })
        .await
    }

    /// Stream realtime stress until Ctrl-C or timeout.
    pub async fn stream_stress(&mut self, should_exit: Arc<AtomicBool>) -> anyhow::Result<()> {
        let mut window = RealtimeStressWindow::default();

        self.stream_realtime(should_exit, |unix, bpm| {
            if bpm == 0 {
                return Ok(());
            }

            let time = format_realtime_time(unix);
            if let Some(score) = window.push(unix, bpm) {
                println!("{time} Stress: {:.2}", score);
            }
            Ok(())
        })
        .await
    }

    pub async fn get_version(&mut self) -> anyhow::Result<()> {
        let mut notifications = self.transport.notifications().await?;
        self.send_command(WhoopPacket::version()).await?;

        let timeout_duration = Duration::from_secs(5);
        match timeout(timeout_duration, notifications.next()).await {
            Ok(Some(notification)) => {
                let packet = match self.generation {
                    WhoopGeneration::Gen4 => WhoopPacket::from_data(notification.value)?,
                    WhoopGeneration::Gen5 => WhoopPacket::from_data_maverick(notification.value)?,
                    WhoopGeneration::Placeholder => {
                        return Err(anyhow!(
                            "WhoopGeneration::Placeholder cannot parse version packets"
                        ));
                    }
                };
                let data = match self.generation {
                    WhoopGeneration::Gen4 => WhoopData::from_packet_gen4(packet)?,
                    WhoopGeneration::Gen5 => WhoopData::from_packet_gen5(packet)?,
                    WhoopGeneration::Placeholder => {
                        return Err(anyhow!(
                            "WhoopGeneration::Placeholder cannot decode version packets"
                        ));
                    }
                };
                if let WhoopData::VersionInfo { harvard, boylston } = data {
                    info!("version harvard {} boylston {}", harvard, boylston);
                }
                Ok(())
            }
            Ok(None) => Err(anyhow!("stream ended unexpectedly")),
            Err(_) => Err(anyhow!("timed out waiting for version notification")),
        }
    }

    pub async fn get_alarm(&mut self) -> anyhow::Result<WhoopData> {
        self.subscribe(self.generation.cmd_from_strap()).await?;
        let mut notifications = self.transport.notifications().await?;
        self.send_command(WhoopPacket::get_alarm_time()).await?;

        let timeout_duration = Duration::from_secs(30);
        match timeout(timeout_duration, notifications.next()).await {
            Ok(Some(notification)) => {
                let packet = WhoopPacket::from_data(notification.value)?;
                let data = WhoopData::from_packet(packet, self.generation)?;
                Ok(data)
            }
            Ok(None) => Err(anyhow!("stream ended unexpectedly")),
            Err(_) => Err(anyhow!("timed out waiting for alarm notification")),
        }
    }

    pub async fn get_battery_level(&mut self) -> anyhow::Result<u8> {
        let value = self
            .transport
            .read(BATTERY_SERVICE_UUID, BATTERY_LEVEL_CHARACTERISTIC_UUID)
            .await?;

        match value.as_slice() {
            [level] => Ok(*level),
            [] => Err(anyhow!("battery characteristic returned an empty payload")),
            bytes => Err(anyhow!(
                "battery characteristic returned unexpected payload: {:?}",
                bytes
            )),
        }
    }
}

fn should_retry_gen5_history_after_full_reconnect(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.contains("idle timeout")
        || msg.contains("timed out waiting for command response")
        || msg.contains("Received non-historical packets after SendHistoricalData")
        || msg.contains(
            "No historical packets observed after SendHistoricalData even after abort/retry",
        )
        || msg.contains("Transfer ended without HistoryComplete metadata")
        || msg.contains("SendHistoricalData rejected")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ble::{BleNotificationStream, BleWriteType};
    use anyhow::anyhow;
    use futures::{StreamExt, channel::mpsc};
    use openwhoop_codec::{
        WhoopPacket,
        constants::{
            CMD_FROM_STRAP_GEN5, CommandNumber, DATA_FROM_STRAP_GEN5, MetadataType, PacketType,
            WhoopGeneration,
        },
    };
    use openwhoop_db::SearchHistory;
    use std::sync::{Mutex, atomic::AtomicBool};

    #[test]
    fn realtime_stress_window_waits_for_a_few_samples() {
        let mut window = RealtimeStressWindow::default();

        for unix in 0..u32::try_from(MIN_REALTIME_STRESS_SAMPLES - 1).unwrap() {
            assert!(window.push(unix, 75).is_none());
        }

        assert_eq!(window.readings.len(), MIN_REALTIME_STRESS_SAMPLES - 1);
        assert_eq!(
            window.push(u32::try_from(MIN_REALTIME_STRESS_SAMPLES - 1).unwrap(), 75),
            Some(10.0)
        );
    }

    #[test]
    fn realtime_stress_window_keeps_rolling_window_size() {
        let mut window = RealtimeStressWindow::default();
        let sample_total = StressCalculator::MIN_READING_PERIOD + 25;

        for unix in 0..u32::try_from(sample_total).unwrap() {
            let _ = window.push(unix, 78);
        }

        assert_eq!(window.readings.len(), StressCalculator::MIN_READING_PERIOD);
    }

    #[derive(Clone, Copy)]
    enum DisconnectScenario {
        BeforeFirstPacket,
        AfterFirstReading,
        NonHistoricalTrafficThenIdle,
        StallThenRetrySucceeds,
        StallTwice,
        StallThenFullReconnectSucceeds,
    }

    #[derive(Clone)]
    struct MockTransport {
        connected: Arc<AtomicBool>,
        notifications_tx: Arc<Mutex<Option<mpsc::UnboundedSender<BleNotification>>>>,
        writes: Arc<Mutex<Vec<u8>>>,
        scenario: DisconnectScenario,
        history_start_count: Arc<Mutex<usize>>,
        connect_count: Arc<Mutex<usize>>,
    }

    impl MockTransport {
        fn new(scenario: DisconnectScenario) -> Self {
            Self {
                connected: Arc::new(AtomicBool::new(true)),
                notifications_tx: Arc::new(Mutex::new(None)),
                writes: Arc::new(Mutex::new(Vec::new())),
                scenario,
                history_start_count: Arc::new(Mutex::new(0)),
                connect_count: Arc::new(Mutex::new(0)),
            }
        }

        fn writes(&self) -> Vec<u8> {
            self.writes.lock().expect("writes mutex poisoned").clone()
        }

        fn emit_cmd_response(
            &self,
            cmd: u8,
            origin_seq: u8,
            result: u8,
            body: Vec<u8>,
        ) -> anyhow::Result<()> {
            let mut data = vec![origin_seq, result];
            data.extend_from_slice(&body);
            let packet = WhoopPacket::new(PacketType::CommandResponse, 0x70, cmd, data);
            self.emit(CMD_FROM_STRAP_GEN5, packet)
        }

        fn emit_history_reading(&self) -> anyhow::Result<()> {
            let mut payload = vec![0u8; 49];
            payload[4..8].copy_from_slice(&1_700_000_000u32.to_le_bytes());
            payload[8..10].copy_from_slice(&512u16.to_le_bytes());
            payload[11] = 72;
            payload[22] = 1;
            payload[23..25].copy_from_slice(&800u16.to_le_bytes());
            payload[30..34].copy_from_slice(&1.0f32.to_le_bytes());
            payload[34..38].copy_from_slice(&0.0f32.to_le_bytes());
            payload[38..42].copy_from_slice(&(-1.0f32).to_le_bytes());
            payload[48] = 97;

            let packet = WhoopPacket::new(PacketType::HistoricalData, 18, 128, payload);
            self.emit(DATA_FROM_STRAP_GEN5, packet)
        }

        fn emit_event_packet(&self) -> anyhow::Result<()> {
            let mut payload = vec![0];
            payload.extend_from_slice(&1_700_000_123u32.to_le_bytes());
            let packet = WhoopPacket::new(PacketType::Event, 0x22, 11, payload);
            self.emit(DATA_FROM_STRAP_GEN5, packet)
        }

        fn emit_metadata(&self, meta_type: MetadataType, end_data: [u8; 8]) -> anyhow::Result<()> {
            let mut payload = Vec::with_capacity(18);
            payload.extend_from_slice(&1_700_000_124u32.to_le_bytes());
            payload.extend_from_slice(&[0; 6]);
            payload.extend_from_slice(&end_data[..4]);
            payload.extend_from_slice(&end_data[4..]);
            let packet = WhoopPacket::new(PacketType::Metadata, 0x33, meta_type as u8, payload);
            self.emit(DATA_FROM_STRAP_GEN5, packet)
        }

        fn emit(&self, uuid: uuid::Uuid, packet: WhoopPacket) -> anyhow::Result<()> {
            let frame = packet.framed_packet_maverick()?;
            let sender = self
                .notifications_tx
                .lock()
                .expect("notifications tx mutex poisoned")
                .as_ref()
                .cloned()
                .ok_or_else(|| anyhow!("notification stream already closed"))?;
            sender
                .unbounded_send(BleNotification { uuid, value: frame })
                .map_err(|_| anyhow!("failed to send mock notification"))?;
            Ok(())
        }

        fn disconnect(&self) {
            self.connected.store(false, Ordering::SeqCst);
            let _ = self
                .notifications_tx
                .lock()
                .expect("notifications tx mutex poisoned")
                .take();
        }
    }

    impl WhoopBleTransport for MockTransport {
        async fn connect(&self) -> anyhow::Result<()> {
            self.connected.store(true, Ordering::SeqCst);
            *self
                .connect_count
                .lock()
                .expect("connect count mutex poisoned") += 1;
            *self
                .history_start_count
                .lock()
                .expect("history start count mutex poisoned") = 0;
            Ok(())
        }

        async fn disconnect(&self) -> anyhow::Result<()> {
            self.disconnect();
            Ok(())
        }

        async fn is_connected(&self) -> anyhow::Result<bool> {
            Ok(self.connected.load(Ordering::SeqCst))
        }

        async fn read(&self, _service: Uuid, _characteristic: Uuid) -> anyhow::Result<Vec<u8>> {
            Ok(vec![])
        }

        async fn subscribe(
            &self,
            _service: uuid::Uuid,
            _characteristic: uuid::Uuid,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn write(
            &self,
            _service: uuid::Uuid,
            _characteristic: uuid::Uuid,
            data: &[u8],
            _write_type: BleWriteType,
        ) -> anyhow::Result<()> {
            let packet = WhoopPacket::from_data_maverick(data.to_vec())?;
            self.writes
                .lock()
                .expect("writes mutex poisoned")
                .push(packet.cmd);

            if !self.connected.load(Ordering::SeqCst) {
                return Err(anyhow!("mock transport disconnected"));
            }

            match CommandNumber::from_u8(packet.cmd) {
                Some(CommandNumber::GetDataRange) => {
                    self.emit_cmd_response(packet.cmd, packet.seq, 1, Vec::new())?;
                }
                Some(CommandNumber::SendHistoricalData) => {
                    let mut history_start_count = self
                        .history_start_count
                        .lock()
                        .expect("history start count mutex poisoned");
                    let connect_count = *self
                        .connect_count
                        .lock()
                        .expect("connect count mutex poisoned");
                    *history_start_count += 1;
                    self.emit_cmd_response(packet.cmd, packet.seq, 2, Vec::new())?;
                    match self.scenario {
                        DisconnectScenario::AfterFirstReading => {
                            self.emit_history_reading()?;
                            self.disconnect();
                        }
                        DisconnectScenario::BeforeFirstPacket => {
                            self.disconnect();
                        }
                        DisconnectScenario::NonHistoricalTrafficThenIdle => {
                            self.emit_event_packet()?;
                        }
                        DisconnectScenario::StallThenRetrySucceeds => {
                            if *history_start_count >= 2 {
                                let end_data = [1, 2, 3, 4, 5, 6, 7, 8];
                                self.emit_history_reading()?;
                                self.emit_metadata(MetadataType::HistoryEnd, end_data)?;
                                self.emit_metadata(MetadataType::HistoryComplete, [0; 8])?;
                            }
                        }
                        DisconnectScenario::StallTwice => {}
                        DisconnectScenario::StallThenFullReconnectSucceeds => {
                            if connect_count >= 1 {
                                let end_data = [8, 7, 6, 5, 4, 3, 2, 1];
                                self.emit_history_reading()?;
                                self.emit_metadata(MetadataType::HistoryEnd, end_data)?;
                                self.emit_metadata(MetadataType::HistoryComplete, [0; 8])?;
                            }
                        }
                    }
                }
                Some(CommandNumber::AbortHistoricalTransmits) => {
                    self.emit_cmd_response(packet.cmd, packet.seq, 1, Vec::new())?;
                }
                Some(CommandNumber::HistoricalDataResult) => {
                    self.emit_cmd_response(packet.cmd, packet.seq, 1, Vec::new())?;
                    match self.scenario {
                        DisconnectScenario::StallThenRetrySucceeds => {}
                        _ => {}
                    }
                }
                _ => {}
            }

            Ok(())
        }

        async fn notifications(&self) -> anyhow::Result<BleNotificationStream> {
            let (tx, rx) = mpsc::unbounded();
            *self
                .notifications_tx
                .lock()
                .expect("notifications tx mutex poisoned") = Some(tx);
            Ok(rx.boxed())
        }
    }

    #[tokio::test]
    async fn sync_history_gen5_persists_trailing_readings_when_strap_disconnects() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let db_check = db.clone();
        let transport = MockTransport::new(DisconnectScenario::AfterFirstReading);
        let mut device =
            WhoopDeviceWith::from_transport(transport.clone(), db, false, WhoopGeneration::Gen5);

        let err = device
            .sync_history(
                Arc::new(AtomicBool::new(false)),
                HistorySyncConfig::from_secs(30, 1),
            )
            .await
            .expect_err("history sync should fail after strap disconnect");

        assert!(
            err.to_string()
                .contains("Whoop disconnected while receiving historical packets")
        );

        let history = db_check
            .search_history(SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].bpm, 72);
        assert_eq!(history[0].rr, vec![800]);

        assert_eq!(
            transport.writes(),
            vec![
                CommandNumber::GetDataRange.as_u8(),
                CommandNumber::SendHistoricalData.as_u8()
            ]
        );
    }

    #[tokio::test]
    async fn sync_history_gen5_disconnect_before_data_is_not_treated_as_empty_history() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let db_check = db.clone();
        let transport = MockTransport::new(DisconnectScenario::BeforeFirstPacket);
        let mut device =
            WhoopDeviceWith::from_transport(transport.clone(), db, false, WhoopGeneration::Gen5);

        let err = device
            .sync_history(
                Arc::new(AtomicBool::new(false)),
                HistorySyncConfig::from_secs(30, 1),
            )
            .await
            .expect_err("history sync should fail after strap disconnect");

        assert!(
            err.to_string()
                .contains("Whoop disconnected while receiving historical packets")
        );

        let history = db_check
            .search_history(SearchHistory::default())
            .await
            .unwrap();
        assert!(history.is_empty());

        assert_eq!(
            transport.writes(),
            vec![
                CommandNumber::GetDataRange.as_u8(),
                CommandNumber::SendHistoricalData.as_u8()
            ]
        );
    }

    #[tokio::test]
    async fn sync_history_gen5_non_historical_packets_are_not_treated_as_empty_history() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let db_check = db.clone();
        let transport = MockTransport::new(DisconnectScenario::NonHistoricalTrafficThenIdle);
        let mut device =
            WhoopDeviceWith::from_transport(transport.clone(), db, false, WhoopGeneration::Gen5);

        let err = device
            .sync_history(
                Arc::new(AtomicBool::new(false)),
                HistorySyncConfig::from_secs(30, 1),
            )
            .await
            .expect_err("history sync should fail after only non-historical traffic");

        assert!(
            err.to_string()
                .contains("Received non-historical packets after SendHistoricalData")
        );

        let history = db_check
            .search_history(SearchHistory::default())
            .await
            .unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn sync_history_gen5_retries_once_after_idle_timeout() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let db_check = db.clone();
        let transport = MockTransport::new(DisconnectScenario::StallThenRetrySucceeds);
        let mut device =
            WhoopDeviceWith::from_transport(transport.clone(), db, false, WhoopGeneration::Gen5);

        device
            .sync_history(
                Arc::new(AtomicBool::new(false)),
                HistorySyncConfig::from_secs(30, 1),
            )
            .await
            .expect("history sync should recover after a single retry");

        let history = db_check
            .search_history(SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].bpm, 72);

        assert_eq!(
            transport.writes(),
            vec![
                CommandNumber::GetDataRange.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::HistoricalDataResult.as_u8(),
            ]
        );
    }

    #[tokio::test]
    async fn sync_history_gen5_stall_after_retry_returns_explicit_error() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let transport = MockTransport::new(DisconnectScenario::StallTwice);
        let mut device =
            WhoopDeviceWith::from_transport(transport.clone(), db, false, WhoopGeneration::Gen5);

        let err = device
            .sync_history(
                Arc::new(AtomicBool::new(false)),
                HistorySyncConfig::from_secs(30, 1),
            )
            .await
            .expect_err("history sync should fail after stalling twice");

        assert!(err.to_string().contains(
            "No historical packets observed after SendHistoricalData even after abort/retry"
        ));

        assert_eq!(
            transport.writes(),
            vec![
                CommandNumber::GetDataRange.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::HistoricalDataResult.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
                CommandNumber::GetDataRange.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::HistoricalDataResult.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
            ]
        );
    }

    #[tokio::test]
    async fn sync_history_gen5_full_reconnect_retry_recovers_after_stale_session() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let db_check = db.clone();
        let transport = MockTransport::new(DisconnectScenario::StallThenFullReconnectSucceeds);
        let mut device =
            WhoopDeviceWith::from_transport(transport.clone(), db, false, WhoopGeneration::Gen5);

        device
            .sync_history(
                Arc::new(AtomicBool::new(false)),
                HistorySyncConfig::from_secs(30, 1).with_exit_on_failure(true),
            )
            .await
            .expect("history sync should recover after a full reconnect retry");

        let history = db_check
            .search_history(SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].bpm, 72);

        assert_eq!(
            transport.writes(),
            vec![
                CommandNumber::GetDataRange.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::HistoricalDataResult.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
                CommandNumber::GetDataRange.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::HistoricalDataResult.as_u8(),
            ]
        );
    }

    #[tokio::test]
    async fn sync_history_gen5_skips_full_reconnect_retry_when_exit_on_failure_is_false() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let transport = MockTransport::new(DisconnectScenario::StallThenFullReconnectSucceeds);
        let mut device =
            WhoopDeviceWith::from_transport(transport.clone(), db, false, WhoopGeneration::Gen5);

        let err = device
            .sync_history(
                Arc::new(AtomicBool::new(false)),
                HistorySyncConfig::from_secs(30, 1),
            )
            .await
            .expect_err("history sync should fail without the forced reconnect retry");

        assert!(err.to_string().contains(
            "No historical packets observed after SendHistoricalData even after abort/retry"
        ));

        assert_eq!(
            transport.writes(),
            vec![
                CommandNumber::GetDataRange.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
                CommandNumber::SendHistoricalData.as_u8(),
                CommandNumber::HistoricalDataResult.as_u8(),
                CommandNumber::AbortHistoricalTransmits.as_u8(),
            ]
        );
    }
}
