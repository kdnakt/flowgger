use flowgger::config::Config;
use flowgger::decoder::Decoder;
use flowgger::encoder::Encoder;
use flowgger::splitter::{Splitter, CapnpSplitter, LineSplitter, NulSplitter, SyslenSplitter};
use coio::Scheduler;
use coio::net::{TcpListener, TcpStream};
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::mpsc::SyncSender;
use super::*;

pub struct TcpCoInput {
    listen: String,
    tcp_config: TcpConfig,
}

impl TcpCoInput {
    pub fn new(config: &Config) -> TcpCoInput {
        let (tcp_config, listen, _timeout) = config_parse(&config);
        TcpCoInput {
            listen: listen,
            tcp_config: tcp_config,
        }
    }
}

impl Input for TcpCoInput {
    fn accept(
        &self,
        tx: SyncSender<Vec<u8>>,
        decoder: Box<Decoder + Send>,
        encoder: Box<Encoder + Send>,
    ) {
        let tcp_config = self.tcp_config.clone();
        let threads = tcp_config.threads;
        let listen: SocketAddr = self.listen.parse().unwrap();
        Scheduler::new()
            .with_workers(threads)
            .run(move || {
                let listener = TcpListener::bind(listen).unwrap();
                for client in listener.incoming() {
                    match client {
                        Ok((client, _addr)) => {
                            let tx = tx.clone();
                            let (decoder, encoder) = (decoder.clone_boxed(), encoder.clone_boxed());
                            let tcp_config = tcp_config.clone();
                            Scheduler::spawn(move || {
                                                 handle_client(client,
                                                               tx,
                                                               decoder,
                                                               encoder,
                                                               tcp_config);
                                             });
                        }
                        Err(_) => {}
                    }
                }
            })
            .unwrap();
    }
}

fn handle_client(
    client: TcpStream,
    tx: SyncSender<Vec<u8>>,
    decoder: Box<Decoder>,
    encoder: Box<Encoder>,
    tcp_config: TcpConfig,
) {
    if let Ok(peer_addr) = client.peer_addr() {
        println!("Connection over TCP from [{}]", peer_addr);
    }
    let reader = BufReader::new(client);
    let splitter = match &tcp_config.framing as &str {
        "capnp" => Box::new(CapnpSplitter) as Box<Splitter<_>>,
        "line" => Box::new(LineSplitter) as Box<Splitter<_>>,
        "syslen" => Box::new(SyslenSplitter) as Box<Splitter<_>>,
        "nul" => Box::new(NulSplitter) as Box<Splitter<_>>,
        _ => panic!("Unsupported framing scheme"),
    };
    splitter.run(reader, tx, decoder, encoder);
}
