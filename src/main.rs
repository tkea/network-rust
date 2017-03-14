
#![feature(mpsc_select)]

#[macro_use]
extern crate serde_derive;
extern crate rand;
extern crate chrono;
extern crate timer;

use std::io;
use std::thread;
use std::sync::mpsc::channel;

use rand::Rng;
use chrono::offset::local::Local;

extern crate network_rust;
use network_rust::localip::get_localip;
use network_rust::peer::{PeerTransmitter, PeerReceiver, PeerUpdate};
use network_rust::bcast::{BcastTransmitter, BcastReceiver};

const PEER_PORT: u16 = 9877;
const BCAST_PORT: u16 = 9876;

use std::collections::HashMap;

const N_FLOORS: usize = 4;

type IP = String;
type ElevatorID = IP;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum RequestStatus {
    Active,
    Pending,
    Inactive,
    Unknown
}

use RequestStatus::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Request {
    floor: usize,
    status: RequestStatus,
    assigned_to: Option<String>,
}

struct RequestHandler {
    pub requests: [Request; N_FLOORS],
}

impl RequestHandler {
    pub fn new() -> Self {
        RequestHandler {
            requests: [
                Request {floor: 0, status: Unknown, assigned_to: None},
                Request {floor: 1, status: Unknown, assigned_to: None},
                Request {floor: 2, status: Unknown, assigned_to: None},
                Request {floor: 3, status: Unknown, assigned_to: None},
            ]
        }
    }

    pub fn merge_request(&mut self, remote_request: Request) {
        let ref floor = remote_request.floor;

        let local_status = self.requests[*floor].status;
        let remote_status = remote_request.status;

        match (local_status, remote_status) {
            (Active, Inactive)  => self.move_to_inactive(&remote_request),
            (Inactive, Pending) => self.move_to_pending(&remote_request),
            (Pending, Active)   => self.move_to_active(&remote_request),
            (Pending, Pending)  => self.update_acknowledgements(&remote_request),
            (Unknown, _)        => self.handle_unknown_local(&remote_request),
            _                   => return,
        }
    }

    fn move_to_active(&mut self, remote: &Request) {
        self.requests[remote.floor].status = Active;

    }

    fn move_to_inactive(&mut self, remote: &Request) {
        self.requests[remote.floor].status = Inactive;
    }

    fn move_to_pending(&mut self, remote: &Request) {
        self.requests[remote.floor].status = Pending;
    }

    fn update_acknowledgements(&mut self, remote: &Request) {
        println!("request was acked");
    }

    fn handle_unknown_local(&mut self, remote: &Request) {
        self.requests[remote.floor] = remote.clone();
    }

    fn print(&self) {
        for request in self.requests.iter() {
            println!("{:?}", request);
        }
    }
}

// ...




fn main() {
    let mut requests = RequestHandler::new();


    let unique = rand::thread_rng().gen::<u16>();

    // Spawn peer transmitter and receiver
    thread::spawn(move || {
        let id = format!("{}:{}", get_localip().unwrap(), unique);
        PeerTransmitter::new(PEER_PORT)
            .expect("Error creating PeerTransmitter")
            .run(&id);
    });
    let (peer_tx, peer_rx) = channel::<PeerUpdate<String>>();
    thread::spawn(move|| {
        PeerReceiver::new(PEER_PORT)
            .expect("Error creating PeerReceiver")
            .run(peer_tx);
    });

    // Spawn broadcast transmitter and receiver
    let (transmit_tx, transmit_rx) = channel::<Request>();
    let (receive_tx, receive_rx) = channel::<Request>();
    thread::spawn(move|| {
        BcastTransmitter::new(BCAST_PORT)
            .expect("Error creating ")
            .run(transmit_rx);
    });
    thread::spawn(move|| {
        BcastReceiver::new(BCAST_PORT)
            .expect("Error creating BcastReceiver")
            .run(receive_tx);
    });

    let t_tx = transmit_tx.clone();

    // Spawn user interface
    thread::spawn(move|| {
        let r1 = Request {
            floor: 0,
            status: Inactive,
            assigned_to: None,
        };
        let r2 = Request {
            floor: 1,
            status: Inactive,
            assigned_to: None,
        };
        let r3 = Request {
            floor: 2,
            status: Inactive,
            assigned_to: None,
        };
        let r4 = Request {
            floor: 3,
            status: Inactive,
            assigned_to: None,
        };

        transmit_tx.send(r1).unwrap();
        transmit_tx.send(r2).unwrap();
        transmit_tx.send(r3).unwrap();
        transmit_tx.send(r4).unwrap();

        loop {
            let mut input = String::new();
            io::stdin().read_line(&mut input)
                .expect("Couldn't read line");

            let active = Request {
                floor: 0,
                status: Active,
                assigned_to: None,
            };

            let pending = Request {
                floor: 1,
                status: Pending,
                assigned_to: None,
            };

            let inactive = Request {
                floor: 2,
                status: Inactive,
                assigned_to: None,
            };

            transmit_tx.send(active).unwrap();
            transmit_tx.send(pending).unwrap();
            transmit_tx.send(inactive).unwrap();

        }
    });

    // Broadcast all orders every T ms
    let (timer_tx, timer_rx) = channel::<()>();
    let timer = timer::Timer::new();
    let timer_guard = timer.schedule_repeating(chrono::Duration::seconds(1), move ||{
        timer_tx.send(()).unwrap();
    });


    // Start infinite loop waiting on either bcast msg or peer update
    loop {
        select! {
            update = peer_rx.recv() => {
                println!("{}", update.unwrap());
            },
            bcast_msg = receive_rx.recv() => {
                let message = bcast_msg.unwrap();
                println!("Got bcast_msg: {:?}", message);

                requests.merge_request(message.clone());
            },
            _ = timer_rx.recv() => {
                for request in requests.requests.iter() {
                    t_tx.send(request.clone()).unwrap();
                }

                requests.print();
            }
        }
    }
}
