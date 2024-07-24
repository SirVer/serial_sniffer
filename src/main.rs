use anyhow::{Context, Result};
use argh::FromArgs;
use crossbeam_channel::{select, unbounded};
use std::fs;
use std::io::Write;
use std::io::{self, Read};
use std::thread;
use std::time::Duration;

#[derive(FromArgs)]
/// Sniff serial communication
struct Args {
    #[argh(option, short = 'c')]
    /// serial where computer is connected.
    com: String,

    #[argh(option, short = 'd')]
    /// serial where device is connected.
    device: String,

    #[argh(option, short = 'b', default = "115200")]
    /// baud rate
    bauds: u32,
}

#[derive(Debug)]
enum Data {
    FromCom(Vec<u8>),
    FromDev(Vec<u8>),
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();
    let mut com_read = serialport::new(&args.com, args.bauds)
        .open()
        .context("Could not open computer")?;
    let mut dev_read = serialport::new(&args.device, args.bauds)
        .open()
        .context("Could not open device")?;

    let mut dev_write = dev_read.try_clone()?;
    let mut com_write = com_read.try_clone()?;

    let (tx_exit, rx_exit) = unbounded();
    ctrlc::set_handler(move || {
        tx_exit.send(true).expect("Always works");
    })
    .expect("Error setting Ctrl-C handler");

    let (tx, rx) = unbounded();
    let tx1 = tx.clone();
    thread::spawn(move || loop {
        let mut buffer: Vec<u8> = vec![0; 1024];
        match com_read.read(buffer.as_mut_slice()) {
            Ok(t) => {
                tx1.send(Data::FromCom(buffer[..t].to_vec()))
                    .expect("Sending works");
                dev_write.write_all(&buffer[..t]).expect("writing works");
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
            Err(e) => eprintln!("{:?}", e),
        }
    });

    thread::spawn(move || loop {
        let mut buffer: Vec<u8> = vec![0; 1024];
        match dev_read.read(buffer.as_mut_slice()) {
            Ok(t) => {
                tx.send(Data::FromDev(buffer[..t].to_vec()))
                    .expect("Sending works");
                com_write.write_all(&buffer[..t]).expect("writing works");
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
            Err(e) => eprintln!("{:?}", e),
        }
    });

    let mut ctod = fs::File::create("ctod.bin")?;
    let mut dtoc = fs::File::create("dtoc.bin")?;
    let mut done = false;
    while !done {
        select! {
            recv(rx) -> msg => {
                match msg.expect("receive worked.") {
                    Data::FromCom(d) => {
                        ctod.write_all(&d)?;
                    }
                    Data::FromDev(d) => {
                        dtoc.write_all(&d)?;
                    }
                }
            }
            recv(rx_exit) -> _ => {
                done = true;
            }
        }
    }
    Ok(())
}
