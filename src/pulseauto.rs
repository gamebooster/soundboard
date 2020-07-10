use anyhow::{anyhow, Result};
use libpulse_binding as pulse;
use log::{error, info, trace, warn};
use pulse::context::State;

pub fn load_module(module_name: &str, args: &str) -> Result<u32> {
    let (mut mainloop, pulse_context) = connect_pulse()?;

    let (sender, receiver): (
        crossbeam_channel::Sender<Result<u32>>,
        crossbeam_channel::Receiver<Result<u32>>,
    ) = crossbeam_channel::unbounded();

    let callback = move |module_index: u32| {
        sender.send(Ok(module_index)).expect("send channel error");
    };

    mainloop.lock();

    let mut introspector = pulse_context.introspect();
    introspector.load_module(module_name, args, callback);

    mainloop.unlock();

    let result = match receiver.recv() {
        Err(err) => Err(anyhow!("Failed to recv from pulse module callback {}", err)),
        Ok(Err(err)) => Err(anyhow!("Failed to load pulse module {}", err)),
        Ok(Ok(module_index)) => Ok(module_index),
    };

    mainloop.stop();
    result
}

pub fn unload_module(loop_module_id: u32) -> Result<()> {
    let (mut mainloop, pulse_context) = connect_pulse()?;

    let (sender, receiver): (
        crossbeam_channel::Sender<bool>,
        crossbeam_channel::Receiver<bool>,
    ) = crossbeam_channel::unbounded();

    let callback = move |result| {
        sender.send(result).expect("channel send error");
    };

    mainloop.lock();

    let mut introspector = pulse_context.introspect();
    introspector.unload_module(loop_module_id, callback);

    mainloop.unlock();

    let result = match receiver.recv() {
        Err(err) => Err(anyhow!("Failed to unload pulse module {}", err)),
        Ok(false) => Err(anyhow!("Failed to unload pulse module {}")),
        Ok(true) => Ok(()),
    };

    mainloop.stop();

    result
}

fn connect_pulse() -> Result<(pulse::mainloop::threaded::Mainloop, pulse::context::Context)> {
    let mut mainloop = pulse::mainloop::threaded::Mainloop::new()
        .ok_or_else(|| anyhow!("Pulse Mainloop Creation failed"))?;

    mainloop
        .start()
        .map_err(|err| anyhow!("Pulse Mainloop Start failed {}", err))?;

    mainloop.lock();

    let mut pulse_context: pulse::context::Context =
        pulse::context::Context::new(&mainloop, "Soundboard")
            .ok_or_else(|| anyhow!("Pulse Connection Callback failed"))?;

    pulse_context
        .connect(None, pulse::context::flags::NOFLAGS, None)
        .map_err(|err| anyhow!("Pulse Mainloop Creation failed {}", err))?;

    mainloop.unlock();

    loop {
        match pulse_context.get_state() {
            State::Ready => {
                trace!("connection: ready");
                break;
            }
            State::Failed => {
                trace!("connection: failed");
                return Err(anyhow!("Failed to connect to Pulse Server: Failed state"));
            }
            State::Terminated => {
                trace!("connection: terminated");
                return Err(anyhow!(
                    "Failed to connect to Pulse Server: Terminated state"
                ));
            }
            State::Connecting => {
                trace!("connection: connecting");
            }
            _ => {
                trace!("connection: unexpected state");
            }
        };

        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    Ok((mainloop, pulse_context))
}
