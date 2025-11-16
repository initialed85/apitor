use gilrs::{EventType, Gilrs};
use std::{sync::Arc, time::Duration};
use tokio::{sync::Mutex, time};
use tokio_stream::StreamExt;

use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral, ScanFilter, WriteType};
use btleplug::platform::Manager;

const PASSWORD_Q: &str = "55aa112064796f7a574f50663035326757565034";

const LEFT_MOTOR: u8 = 0x06;
const RIGHT_MOTOR: u8 = 0x07;

#[allow(clippy::never_loop)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let mut gilrs = Gilrs::new().unwrap();

    for (_id, gamepad) in gilrs.gamepads() {
        println!("{} is {:?}", gamepad.name(), gamepad.power_info());
    }

    let manager = Manager::new().await?;
    let adapter_list = manager.adapters().await?;
    let adapter = adapter_list.first().expect("failed to find adapter");

    let adapter_info = adapter.adapter_info().await?;

    println!("scanning on {}...", adapter_info);

    let mut relevant_peripheral = None;
    let mut relevant_service = None;
    let mut write_characteristic = None;
    let mut notify_characteristic = None;

    adapter
        .start_scan(ScanFilter::default())
        .await
        .expect("scan failed");

    loop {
        time::sleep(Duration::from_millis(10)).await;

        let peripherals = adapter.peripherals().await?;
        if peripherals.is_empty() {
            continue;
        }

        // All peripheral devices in range
        for peripheral in peripherals.iter() {
            let peripheral = peripheral.clone();

            let properties = peripheral.properties().await?;

            if properties.clone().unwrap().local_name.is_none() {
                continue;
            }

            let local_name = properties
                .clone()
                .unwrap()
                .local_name
                .unwrap_or(String::from("(peripheral name unknown)"));

            if !local_name.starts_with("Apitor") {
                continue;
            }

            let mut is_connected = peripheral.is_connected().await?;
            if !is_connected && let Err(err) = peripheral.connect().await {
                println!("failed to connect to {:?} because {:?}", peripheral, err);
                continue;
            }

            is_connected = peripheral.is_connected().await?;
            if !is_connected {
                println!("failed to {:?} is not connected", peripheral);
                continue;
            }

            peripheral.discover_services().await?;

            for service in peripheral.services() {
                for characteristic in service.clone().characteristics {
                    if characteristic
                        .properties
                        .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
                        && write_characteristic.is_none()
                    {
                        write_characteristic = Some(characteristic);
                    } else if characteristic.properties.contains(CharPropFlags::NOTIFY)
                        && notify_characteristic.is_none()
                    {
                        notify_characteristic = Some(characteristic);
                    }
                }

                if write_characteristic.is_some() && notify_characteristic.is_some() {
                    relevant_service = Some(service);
                    break;
                }
            }

            if relevant_service.is_some() {
                relevant_peripheral = Some(peripheral);
            }
        }

        if write_characteristic.is_some()
            || notify_characteristic.is_some()
            || relevant_service.is_some()
            || relevant_peripheral.is_some()
        {
            break;
        }
    }

    let write_characteristic = write_characteristic.clone().unwrap();
    let notify_characteristic = notify_characteristic.clone().unwrap();
    let relevant_service = relevant_service.clone().unwrap();
    let relevant_peripheral = relevant_peripheral.clone().unwrap();

    println!();
    println!("{:?}", relevant_peripheral);
    println!("\t{:?}", relevant_service);
    println!("\t\t{:?}", notify_characteristic);
    println!("\t\t{:?}", write_characteristic);

    let password_bytes = hex::decode(PASSWORD_Q)?;

    println!();
    println!("sending authentication...");
    relevant_peripheral
        .write(
            &write_characteristic,
            &password_bytes,
            WriteType::WithoutResponse,
        )
        .await?;
    println!("sent authentication.");
    time::sleep(Duration::from_millis(10)).await;

    let safe_left_colour = Arc::new(Mutex::new(0x01));
    let safe_right_colour = Arc::new(Mutex::new(0x02));

    let safe_left_colour_clone = safe_left_colour.clone();
    let safe_right_colour_clone = safe_right_colour.clone();

    let mut last_left_speed = 0;
    let mut last_right_speed = 0;

    let safe_left_speed = Arc::new(Mutex::new(0));
    let safe_right_speed = Arc::new(Mutex::new(0));

    let safe_left_speed_clone = safe_left_speed.clone();
    let safe_right_speed_clone = safe_right_speed.clone();

    relevant_peripheral
        .subscribe(&notify_characteristic)
        .await?;

    tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_millis(100)).await;

            let mut left_colour = *safe_left_colour_clone.lock().await;
            let mut right_colour = *safe_right_colour_clone.lock().await;

            let mut left_speed = *safe_left_speed_clone.lock().await;
            let mut right_speed = *safe_right_speed_clone.lock().await;

            let mut notification_stream =
                relevant_peripheral.notifications().await.unwrap().take(1);

            let notification = notification_stream.next().await;

            println!(
                "left_speed:\t{:?}\tright_speed:\t{:?}\t{:?}",
                left_speed, right_speed, notification
            );

            //
            // colours
            //

            left_colour += 1;
            right_colour += 1;

            if !(0x1..=0x16).contains(&left_colour) {
                left_colour = 0x1;
            }

            if left_speed > 0 {
                left_colour = 0x01;
            } else if left_speed < 0 {
                left_colour = 0x06;
            } else if right_speed != 0 {
                left_colour = 0x04;
            }

            if !(0x1..=0x16).contains(&right_colour) {
                right_colour = 0x01;
            }

            if right_speed > 0 {
                right_colour = 0x01;
            } else if right_speed < 0 {
                right_colour = 0x06;
            } else if left_speed != 0 {
                right_colour = 0x04;
            }

            *safe_left_colour_clone.lock().await = left_colour;
            *safe_right_colour_clone.lock().await = right_colour;

            //
            // left led
            //

            let left_led = 0x01;

            let command_hex = format!("55aa04{left_led:02x}{left_colour:02x}0000");
            let command_bytes = hex::decode(&command_hex).unwrap();

            relevant_peripheral
                .write(
                    &write_characteristic,
                    &command_bytes,
                    WriteType::WithoutResponse,
                )
                .await
                .unwrap();

            //
            // right led
            //

            let right_led = 0x02;

            let command_hex = format!("55aa04{right_led:02x}{right_colour:02x}0000");
            let command_bytes = hex::decode(&command_hex).unwrap();

            relevant_peripheral
                .write(
                    &write_characteristic,
                    &command_bytes,
                    WriteType::WithoutResponse,
                )
                .await
                .unwrap();

            //
            // left motor
            //

            if left_speed != last_left_speed {
                if left_speed == 0 {
                    let stop_cmd = hex::decode(format!("55aa03{LEFT_MOTOR:02x}0000")).unwrap();
                    relevant_peripheral
                        .write(&write_characteristic, &stop_cmd, WriteType::WithoutResponse)
                        .await
                        .unwrap();
                } else {
                    let direction = { if left_speed > 0 { 0x01 } else { 0x02 } };

                    if left_speed < 0 {
                        left_speed = -left_speed;
                    }

                    let command_hex =
                        format!("55aa03{LEFT_MOTOR:02x}{direction:02x}{left_speed:02x}");
                    let command_bytes = hex::decode(&command_hex).unwrap();

                    relevant_peripheral
                        .write(
                            &write_characteristic,
                            &command_bytes,
                            WriteType::WithoutResponse,
                        )
                        .await
                        .unwrap();
                }

                last_left_speed = left_speed;
            }

            //
            // right motor
            //

            if right_speed != last_right_speed {
                if right_speed == 0 {
                    let stop_cmd = hex::decode(format!("55aa03{RIGHT_MOTOR:02x}0000")).unwrap();
                    relevant_peripheral
                        .write(&write_characteristic, &stop_cmd, WriteType::WithoutResponse)
                        .await
                        .unwrap();
                } else {
                    let direction = { if right_speed < 0 { 0x01 } else { 0x02 } };

                    if right_speed < 0 {
                        right_speed = -right_speed;
                    }

                    let command_hex =
                        format!("55aa03{RIGHT_MOTOR:02x}{direction:02x}{right_speed:02x}");
                    let command_bytes = hex::decode(&command_hex).unwrap();

                    relevant_peripheral
                        .write(
                            &write_characteristic,
                            &command_bytes,
                            WriteType::WithoutResponse,
                        )
                        .await
                        .unwrap();
                }

                last_right_speed = right_speed;
            }
        }
    });

    loop {
        while let Some(event) = gilrs.next_event() {
            if let EventType::AxisChanged(axis, value, _code) = event.event {
                match axis {
                    gilrs::Axis::LeftStickY => {
                        *safe_left_speed.lock().await = (value * 12.0) as i32;
                    }
                    gilrs::Axis::RightStickY => {
                        *safe_right_speed.lock().await = (value * 12.0) as i32;
                    }
                    _ => {}
                }
            }
        }
    }
}
