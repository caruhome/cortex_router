use cerk::kernel::{BrokerEvent, Config, IncomingCloudEvent, CloudEventRoutingArgs, DeliveryGuarantee};
use cerk::runtime::channel::{BoxedReceiver, BoxedSender};
use cerk::runtime::{InternalServerFn, InternalServerFnRefStatic, InternalServerId};
use cloudevents::{EventBuilder, EventBuilderV10};
use gpio::{GpioIn,GpioValue};
use gpio::dummy::DummyGpioIn;
use gpio::sysfs::SysFsGpioInput;
use std::thread;
use std::time::{Duration,SystemTime, UNIX_EPOCH};
use std::fmt::Debug;
use uuid::Uuid;
use chrono::Utc;

#[derive(Default)]
struct GpioConfig {
    gpio_num: u16,
    interval: Duration,
    dummy: bool,
}

fn listen_to_dummy_gpio(id: InternalServerId, config: GpioConfig, sender_to_kernel: BoxedSender) {
    let gpio = DummyGpioIn::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() % 2 == 0
    });
    thread::spawn(move || listen_to_gpio(id, Box::new(gpio), config, sender_to_kernel));
}

fn listen_to_sysfs_gpio(id: InternalServerId, config: GpioConfig, sender_to_kernel: BoxedSender) {
    let gpio = SysFsGpioInput::open(config.gpio_num).unwrap();
    thread::spawn(move || listen_to_gpio(id, Box::new(gpio), config, sender_to_kernel));
}

fn new_event(id: &InternalServerId, name: String) -> BrokerEvent {
    let event_id = Uuid::new_v4();
    BrokerEvent::IncomingCloudEvent(
    IncomingCloudEvent {
        routing_id: event_id.to_string(),
        incoming_id: id.clone(),
        cloud_event: EventBuilderV10::new()
            .id(event_id.to_string())
            .ty(name)
            .time(Utc::now())
            .source("http://example.com/dummy.sequence-generator")
            .build()
            .unwrap(),
        args: CloudEventRoutingArgs { delivery_guarantee: DeliveryGuarantee::BestEffort},
    }
)
    
}

fn listen_to_gpio<T: Debug>(id: InternalServerId, mut gpio: Box<dyn GpioIn<Error = T>>, config: GpioConfig, sender_to_kernel: BoxedSender) {
    let mut last_value = GpioValue::Low;
    loop {
        let value = gpio.read_value().unwrap();
        if value == GpioValue::Low && last_value == GpioValue::High {
            let event = new_event(&id, "cortex.button_press.ended".into());
            sender_to_kernel.send(event)
        } else if value == GpioValue::High && last_value == GpioValue::Low {
            let event = new_event(&id, "cortex.button_press.started".into());
            sender_to_kernel.send(event)
        }
        last_value = value;
        thread::sleep(config.interval);
    }
}

fn get_gpio_config(id: &InternalServerId, config: Config) -> GpioConfig {
    match config {
        Config::HashMap(ref config_map) => {
            let mut gpio_config = GpioConfig::default();
            gpio_config.gpio_num = if let Config::U8(gpio_num) = &config_map["gpio_num"] {
                info!("gpio_num={}", gpio_num);
                 (*gpio_num).into()
            } else {
                panic!("{} received invalide config, no gpio_num as int", id);
            };

            gpio_config.interval  = if let Some(Config::U32(interval_millis)) = config_map.get("interval_millis") {
                Duration::from_millis((*interval_millis).into())
            } else {
                Duration::from_millis(100)
            };

            gpio_config.dummy  = if let Some(Config::Bool(dummy)) = config_map.get("dummy") {
                *dummy
            } else {
                false
            };
            return gpio_config;
        },
        _ => panic!("{} invalid config", id)
    }
}

pub fn port_touch_start(id: InternalServerId, inbox: BoxedReceiver, sender_to_kernel: BoxedSender) {

    info!("start touch port with id {}", id);
    let mut initialized = false;

    loop {
        match inbox.receive() {
            BrokerEvent::Init => {
                info!("{} initiated", id);
            }
            BrokerEvent::ConfigUpdated(config, _) => {
                info!("{} received ConfigUpdated", &id);
                if initialized {
                    error!("config for touch port ({}) can't be updated dynamically", id);
                } else {
                    let gpio_config = get_gpio_config(&id, config);
                    if gpio_config.dummy {
                        listen_to_dummy_gpio(id.clone(), gpio_config, sender_to_kernel.clone_boxed())
                    } else {
                        listen_to_sysfs_gpio(id.clone(), gpio_config, sender_to_kernel.clone_boxed())
                    }
                    initialized = true;
                }
            }
            broker_event => warn!("event {} not implemented", broker_event),
        }
    }
}

/// This is the pointer for the main function to start the port.
pub static PORT_TOUCH: InternalServerFnRefStatic = &(port_touch_start as InternalServerFn);
