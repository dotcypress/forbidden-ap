use crate::captive::CaptivePortal;
use dns::*;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::prelude::Peripherals,
    http::{
        server::{Configuration, EspHttpServer},
        Method,
    },
    io::{EspIOError, Write},
    ipv4::{self, Mask, RouterConfiguration, Subnet},
    log::EspLogger,
    netif::{EspNetif, NetifConfiguration, NetifStack},
    nvs::EspDefaultNvsPartition,
    sys::{self, EspError},
    wifi::{self, AccessPointConfiguration, EspWifi, WifiDriver},
};
use std::{
    net::Ipv4Addr,
    str::FromStr,
    thread::{self, sleep},
    time::Duration,
};

mod captive;
mod dns;

pub const IP_ADDRESS: Ipv4Addr = Ipv4Addr::new(192, 168, 55, 1);

fn main() -> Result<(), EspError> {
    unsafe {
        sys::nvs_flash_init();
    }
    sys::link_patches();
    EspLogger::initialize_default();

    let event_loop = EspSystemEventLoop::take()?;
    let peripherals = Peripherals::take()?;

    let wifi_driver = WifiDriver::new(
        peripherals.modem,
        event_loop.clone(),
        EspDefaultNvsPartition::take().ok(),
    )?;
    let mut wifi = EspWifi::wrap_all(
        wifi_driver,
        EspNetif::new(NetifStack::Sta)?,
        EspNetif::new_with_conf(&NetifConfiguration {
            ip_configuration: Some(ipv4::Configuration::Router(RouterConfiguration {
                subnet: Subnet {
                    gateway: IP_ADDRESS,
                    mask: Mask(24),
                },
                dhcp_enabled: true,
                dns: Some(IP_ADDRESS),
                secondary_dns: Some(IP_ADDRESS),
            })),
            ..NetifConfiguration::wifi_default_router()
        })?,
    )
    .expect("WiFi init failed");

    let ssid = heapless::String::from_str(env!("SSID")).unwrap();

    wifi.set_configuration(&wifi::Configuration::AccessPoint(
        AccessPointConfiguration {
            ssid,
            auth_method: wifi::AuthMethod::None,
            ..Default::default()
        },
    ))?;
    wifi.start()?;

    let mut dns = SimpleDns::try_new(IP_ADDRESS).expect("DNS server init failed");
    thread::spawn(move || loop {
        dns.poll().ok();
        sleep(Duration::from_millis(50));
    });

    let config = Configuration::default();
    let mut server = EspHttpServer::new(&config).expect("HTTP server init failed");
    CaptivePortal::attach(&mut server, IP_ADDRESS).expect("Captive portal attach failed");

    server.fn_handler("/", Method::Get, move |request| -> Result<(), EspIOError> {
        request
            .into_ok_response()?
            .write_all(include_bytes!("web/index.html"))?;
        Ok(())
    })?;

    server.fn_handler(
        "/rr.gif",
        Method::Get,
        move |request| -> Result<(), EspIOError> {
            request
                .into_response(200, None, &[("Content-Type", "image/gif")])?
                .write_all(include_bytes!("web/rr.gif"))?;
            Ok(())
        },
    )?;

    loop {
        sleep(Duration::from_millis(50));
    }
}
