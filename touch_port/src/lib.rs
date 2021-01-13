#[macro_use]
extern crate log;

use cerk::kernel::{
    BrokerEvent, CloudEventRoutingArgs, Config, ConfigHelpers, DeliveryGuarantee, IncomingCloudEvent,
};
use cerk::runtime::channel::{BoxedReceiver, BoxedSender};
use cerk::runtime::{InternalServerFn, InternalServerFnRefStatic, InternalServerId};
use chrono::Utc;
use cloudevents::{EventBuilder, EventBuilderV10};
use gpio::sysfs::SysFsGpioInput;
use gpio::{GpioIn, GpioValue};
use std::thread;
use std::time::{Duration};
use uuid::Uuid;
use anyhow::{Result};

const DEFAULT_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Default)]
struct GpioConfig {
    gpio_num: u16,
    interval: Duration,
}

fn parse_config(config: Config) -> Result<GpioConfig> {
    return Ok(GpioConfig {
        gpio_num:config.get_op_val_u8("gpio_num")?
            .unwrap()
            .into(),
        interval: config.get_op_val_u32("interval_millis")?
            .map(|v| Duration::from_millis(v.into()))
            .unwrap_or(DEFAULT_INTERVAL)
    });
}

fn listen_to_gpio(id: InternalServerId, config: GpioConfig, sender_to_kernel: BoxedSender) {
    thread::spawn(move || {
        let mut gpio = SysFsGpioInput::open(config.gpio_num).unwrap();
        let mut last_value = GpioValue::Low;
        loop {
            let value = gpio.read_value().unwrap();
            match (value, last_value) {
                (GpioValue::High, GpioValue::Low) => sender_to_kernel.send(new_event(&id, "io.caru.device.button_press.started".into())),
                (GpioValue::Low, GpioValue::High) => sender_to_kernel.send(new_event(&id, "io.caru.device.button_press.ended".into())),
                _ => {}
            }
            last_value = value;
            thread::sleep(config.interval);
        }
    });
}

fn new_event(id: &InternalServerId, name: String) -> BrokerEvent {
    let event_id = Uuid::new_v4();
    BrokerEvent::IncomingCloudEvent(IncomingCloudEvent {
        routing_id: event_id.to_string(),
        incoming_id: id.clone(),
        cloud_event: EventBuilderV10::new()
            .id(event_id.to_string())
            .ty(name)
            .time(Utc::now())
            .source("crn:io.caru.device.button")
            .build()
            .unwrap(),
        args: CloudEventRoutingArgs {
            delivery_guarantee: DeliveryGuarantee::BestEffort,
        },
    })
}

pub fn port_touch_start(id: InternalServerId, inbox: BoxedReceiver, sender_to_kernel: BoxedSender) {
    info!("start touch port with id {}", id);
    loop {
        match inbox.receive() {
            BrokerEvent::Init => {
                info!("{} initiated", id);
            }
            BrokerEvent::ConfigUpdated(config, _) => {
                info!("{} received ConfigUpdated", &id);
                let gpio_config = parse_config(config).unwrap();
                listen_to_gpio(id.clone(), gpio_config, sender_to_kernel.clone_boxed());
            }
            broker_event => warn!("event {} not implemented", broker_event),
        }
    }
}

pub static PORT_TOUCH: InternalServerFnRefStatic = &(port_touch_start as InternalServerFn);
