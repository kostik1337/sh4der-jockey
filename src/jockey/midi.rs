use midir::{Ignore, MidiInput, MidiInputConnection};
use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver},
    time::Instant,
};

pub struct Midi {
    pub conn: Option<MidiInputConnection<()>>,
    pub queue: Option<Receiver<[u8; 3]>>,
    pub sliders: [f32; 8],
    pub buttons: [Instant; 8],
    pub bindings: HashMap<[u8; 2], u8>,
}

impl Midi {
    pub fn new() -> Self {
        let conn = None;
        let queue = None;
        let sliders = [0.0; 8];
        let buttons = [Instant::now(); 8];
        let bindings = HashMap::new();

        let mut this = Self {
            conn,
            queue,
            sliders,
            buttons,
            bindings,
        };

        this.connect();
        this
    }

    pub fn connect(&mut self) {
        let mut midi_in = MidiInput::new("Sh4derJockey").unwrap();
        midi_in.ignore(Ignore::None);

        // Get an input port (read from console if multiple are available)
        let in_ports = midi_in.ports();
        let in_port = match in_ports.len() {
            0 => {
                println!("Failed to find midi input port.");
                return;
            }
            1 => {
                println!(
                    "Choosing the only available input port: {}",
                    midi_in.port_name(&in_ports[0]).unwrap()
                );
                &in_ports[0]
            }
            _ => {
                println!("\nAvailable input ports:");
                for (i, p) in in_ports.iter().enumerate() {
                    println!("{}: {}", i, midi_in.port_name(p).unwrap());
                }
                todo!()
            }
        };

        let (tx, rx) = channel();
        let conn = midi_in
            .connect(
                in_port,
                "sh4der-jockey-read-input",
                move |stamp, message, _| {
                    println!("{}: {:?} (len = {})", stamp, message, message.len());
                    let mut out = [0; 3];
                    out.copy_from_slice(message);
                    tx.send(out).unwrap();
                },
                (),
            )
            .ok();

        self.conn = conn;
        self.queue = Some(rx);
    }

    pub fn handle_input(&mut self) {
        if let Some(queue) = &mut self.queue {
            for msg in queue.try_iter() {
                println!(" > {:?} (len = {})", msg, msg.len());
                match self.bindings.get(&msg[..2]) {
                    Some(&id @ 0..=7) => {
                        self.sliders[id as usize] = (msg[2] as f32) / 127.0;
                    }

                    Some(&id @ 8..=15) => {
                        self.buttons[(id - 8) as usize] = Instant::now();
                    }

                    _ => (),
                }
            }
        }
    }

    pub fn bind(&mut self, key: [u8; 2], id: u8) {
        self.bindings.insert(key, id);
    }
}
