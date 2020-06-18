extern crate getopts;
extern crate libxm;
extern crate sdl2;

use getopts::Options;
use libxm::XMContext;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use std::fs::File;
use std::io::Read;
use std::env;
use std::sync::mpsc::Sender;

struct MyCallback {
    xm: XMContext,
    last_loop_count: u8,
    loop_tx: Sender<()>
}

impl AudioCallback for MyCallback {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        self.xm.generate_samples(out);

        let loop_count = self.xm.loop_count();

        if loop_count != self.last_loop_count {
            self.last_loop_count = loop_count;

            // Signal the driver that a loop has occured
            self.loop_tx.send(()).unwrap();
        }
    }
}

fn play_audio(contents: &[u8], rate: u32, max_loops: u8) {
    use std::sync::mpsc::channel;

    let _sdl = sdl2::init().audio().unwrap();

    let (loop_tx, loop_rx) = channel();

    let desired_spec = AudioSpecDesired {
        freq: Some(rate as i32),
        channels: Some(2),
        samples: None
    };

    let device = AudioDevice::open_playback(None, desired_spec, |spec| {
        let mut xm = XMContext::new(&contents, spec.freq as u32).unwrap();
        xm.set_max_loop_count(max_loops);

        println!("Module name: {}", String::from_utf8_lossy(xm.module_name()));
        println!("Tracker: {}", String::from_utf8_lossy(xm.tracker_name()));
        println!("Channels: {}", xm.number_of_channels());
        println!("Module length: {}", xm.module_length());
        println!("Patterns: {}", xm.number_of_patterns());
        println!("Instruments: {}", xm.number_of_instruments());

        MyCallback {
            xm: xm,
            last_loop_count: 0,
            loop_tx: loop_tx
        }
    }).unwrap();

    device.resume();

    for _ in 0..max_loops {
        // Block until the song has looped
        loop_rx.recv().unwrap();
    }
}


fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options] FILE", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("r", "rate", "Set the output rate", "RATE");
    opts.optopt("l", "loops", "Set the maximum number of loops", "LOOPS");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };

    let input = if !matches.free.is_empty() {
        matches.free[0].clone()
    } else {
        print_usage(&program, opts);
        return;
    };

    let rate = match matches.opt_str("r") {
        Some(s) => s.parse().unwrap(),
        None => 48000
    };

    let max_loops = match matches.opt_str("l") {
        Some(s) => s.parse().unwrap(),
        None => 1
    };

    let mut contents = Vec::new();
    File::open(&input).unwrap().read_to_end(&mut contents).unwrap();

    play_audio(&contents, rate, max_loops);
}
