
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
use std::string::String;
struct ElevatorPositions {
    positions: HashMap<String, usize>,

}
impl ElevatorPositions {
    pub fn new() -> Self {
        ElevatorPositions {
            positions: HashMap::new()
        }
    }
    fn update(&mut self, ip: String, floor: usize) {
        self.positions.insert(ip, floor);
    }
    pub fn remove(&mut self, ip: &String) {
        self.positions.remove(ip);
    }
}

/*#[derive(Serialize, Deserialize, Debug)]
struct MyPacket {
    msg: String,
    timestamp: i64,
}*/



#[derive(Serialize, Deserialize, Debug)]
enum Floor {
    At(usize),
    Between,
}

#[derive(Serialize, Deserialize, Debug)]
enum Order {
    CallUp(Floor),
    CallDown(Floor),
    Internal(Floor)
}

#[derive(Serialize, Deserialize, Debug)]
enum RequestStatus {
    Active(Order),
    Pending(Order),
    Inactive(Order),
    Unknown(Order)
}


//const N_FLOORS: usize = 4;
#[derive(Serialize, Deserialize, Debug)]
struct OrderData {
    orders_up: [bool; 4],
    orders_down: [bool; 4],
}

impl OrderData {
    fn new() -> Self {
        OrderData {
            orders_up: [false,false,false,false],
            orders_down: [false,false,false,false],
        }
    }

    fn merge_request(&mut self, message: RequestStatus) {
        match message {
            RequestStatus::Active(_) => {},
            RequestStatus::Pending(_) => {},
            RequestStatus::Inactive(_) => {},
            _ => {},
        }
    }

    fn print(&self) {
        println!("up:\t{:?}", self.orders_up);
        println!("down:\t{:?}", self.orders_down);
    }
}




fn main() {
    let mut orders = OrderData::new();


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
    let (transmit_tx, transmit_rx) = channel::<RequestStatus>();
    let (receive_tx, receive_rx) = channel::<RequestStatus>();
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
        loop {
            let mut input = String::new();
            io::stdin().read_line(&mut input)
                .expect("Couldn't read line");
            /*transmit_tx.send(MyPacket {
                msg:       input.trim().to_string(),
                timestamp: Local::now().timestamp(),
            }).unwrap();*/

            transmit_tx.send(RequestStatus::Active(Order::CallUp(Floor::At(0)))).unwrap();

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

                orders.merge_request(message);
            },
            _ = timer_rx.recv() => {
                for (floor, &ordered) in orders.orders_up.iter().enumerate() {
                    let message = if ordered {
                        RequestStatus::Active(Order::CallUp(Floor::At(floor)))
                    } else {
                        RequestStatus::Inactive(Order::CallUp(Floor::At(floor)))
                    };

                    t_tx.send(message).unwrap();
                }

                orders.print();
            }
        }
    }
}
