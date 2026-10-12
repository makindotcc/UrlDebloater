//! Clipboard change notifications from the compositor via data-control.
//!
//! Polling the clipboard means fetching its whole content every cycle, so the
//! app only reacts to `selection` events here and falls back to polling when
//! the compositor offers no data-control protocol.
//!
//! Author: Deepseek

use std::thread;

use anyhow::Context;
use log::{debug, warn};
use tokio::sync::mpsc;
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle,
    globals::{GlobalList, GlobalListContents, registry_queue_init},
    protocol::{wl_registry::WlRegistry, wl_seat::WlSeat},
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{
        self as ext_device, Event as ExtDeviceEvent, ExtDataControlDeviceV1,
    },
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{
        self as wlr_device, Event as WlrDeviceEvent, ZwlrDataControlDeviceV1,
    },
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
};

pub fn watch() -> Option<mpsc::UnboundedReceiver<()>> {
    let (tx, rx) = mpsc::unbounded_channel();
    let (mut queue, mut state) = match setup(tx) {
        Ok(pair) => pair,
        Err(err) => {
            warn!("Clipboard change events unavailable ({err:#}); polling instead");
            return None;
        }
    };

    thread::Builder::new()
        .name("clipboard-events".into())
        .spawn(move || {
            loop {
                if let Err(err) = queue.blocking_dispatch(&mut state) {
                    warn!("Clipboard change events stopped ({err}); polling instead");
                    return;
                }
                if state.stop {
                    warn!("Clipboard data-control device invalidated; polling instead");
                    return;
                }
            }
        })
        .map_err(|err| warn!("Failed to spawn clipboard event thread ({err}); polling instead"))
        .ok()?;
    Some(rx)
}

fn setup(tx: mpsc::UnboundedSender<()>) -> anyhow::Result<(EventQueue<State>, State)> {
    let connection = Connection::connect_to_env().context("connect to Wayland")?;
    let (globals, queue) = registry_queue_init::<State>(&connection).context("list globals")?;
    let queue_handle = queue.handle();
    let seat: WlSeat = globals
        .bind(&queue_handle, 1..=9, ())
        .context("bind wl_seat")?;
    let device = bind_device(&globals, &queue_handle, &seat)?;

    Ok((
        queue,
        State {
            tx,
            device,
            selection: None,
            stop: false,
        },
    ))
}

fn bind_device(
    globals: &GlobalList,
    queue_handle: &QueueHandle<State>,
    seat: &WlSeat,
) -> anyhow::Result<Device> {
    match globals.bind::<ExtDataControlManagerV1, _, _>(queue_handle, 1..=1, ()) {
        Ok(manager) => {
            debug!("Clipboard change events from ext-data-control v1");
            Ok(Device::Ext(manager.get_data_device(seat, queue_handle, ())))
        }
        Err(ext_err) => {
            match globals.bind::<ZwlrDataControlManagerV1, _, _>(queue_handle, 1..=2, ()) {
                Ok(manager) => {
                    debug!("Clipboard change events from zwlr-data-control v1");
                    Ok(Device::Zwlr(manager.get_data_device(
                        seat,
                        queue_handle,
                        (),
                    )))
                }
                Err(wlr_err) => Err(anyhow::anyhow!(
                    "no data-control manager (ext: {ext_err}, zwlr: {wlr_err})"
                )),
            }
        }
    }
}

struct State {
    tx: mpsc::UnboundedSender<()>,
    device: Device,
    selection: Option<Offer>,
    stop: bool,
}

impl State {
    fn wake(&self) {
        let _ = self.tx.send(());
    }

    fn set_selection(&mut self, offer: Option<Offer>) {
        let previous = self.selection.take();
        self.selection = offer;
        if let Some(previous) = previous {
            previous.destroy();
        }
    }
}

enum Device {
    Ext(ExtDataControlDeviceV1),
    Zwlr(ZwlrDataControlDeviceV1),
}

enum Offer {
    Ext(ExtDataControlOfferV1),
    Zwlr(ZwlrDataControlOfferV1),
}

impl Offer {
    fn destroy(&self) {
        match self {
            Offer::Ext(offer) => offer.destroy(),
            Offer::Zwlr(offer) => offer.destroy(),
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: <WlRegistry as wayland_client::Proxy>::Event,
        _data: &GlobalListContents,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        _event: <WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ExtDataControlManagerV1,
        _event: <ExtDataControlManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrDataControlManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrDataControlManagerV1,
        _event: <ZwlrDataControlManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlOfferV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ExtDataControlOfferV1,
        _event: <ExtDataControlOfferV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrDataControlOfferV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrDataControlOfferV1,
        _event: <ZwlrDataControlOfferV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &ExtDataControlDeviceV1,
        event: ExtDeviceEvent,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            ExtDeviceEvent::Selection { id } => {
                state.set_selection(id.map(Offer::Ext));
                state.wake();
            }
            ExtDeviceEvent::Finished => state.stop = true,
            _ => {}
        }
    }

    wayland_client::event_created_child!(State, ExtDataControlDeviceV1, [
        ext_device::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ZwlrDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrDataControlDeviceV1,
        event: WlrDeviceEvent,
        _data: &(),
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            WlrDeviceEvent::Selection { id } => {
                state.set_selection(id.map(Offer::Zwlr));
                state.wake();
            }
            WlrDeviceEvent::Finished => state.stop = true,
            _ => {}
        }
    }

    wayland_client::event_created_child!(State, ZwlrDataControlDeviceV1, [
        wlr_device::EVT_DATA_OFFER_OPCODE => (ZwlrDataControlOfferV1, ()),
    ]);
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(offer) = self.selection.take() {
            offer.destroy();
        }
        match &self.device {
            Device::Ext(device) => device.destroy(),
            Device::Zwlr(device) => device.destroy(),
        }
    }
}
