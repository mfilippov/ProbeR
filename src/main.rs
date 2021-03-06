extern crate time;

use std::os;
use std::io::net::udp::UdpSocket;
use std::io::net::ip::{Ipv4Addr, SocketAddr};
use std::io::net::ip::IpAddr;
use std::num::ToPrimitive;
use time::*;

static PACKET_STATISTICS_INTERVAL: i32 = 50000;
static MAX_PID_COUNT: usize = 8192;

fn get_pid_cc(pid_name: &[Option<u16>], pid_cc: &[Option<u16>], pid: u16) -> Option<u16> {
    for i in range(0us, MAX_PID_COUNT) {
        if pid_name[i].is_some() && pid_name[i].unwrap() == pid {
            return pid_cc[i];
        }
    }
    return None;
}

fn set_pid_cc(pid_name: &mut[Option<u16>], pid_cc: &mut[Option<u16>], pid: u16, cc: u16) {
    let mut index: Option<usize> = None; 
    for i in range(0us, MAX_PID_COUNT) {
        if pid_name[i].is_some() && pid_name[i].unwrap() == pid {
            index = Some(i);
            break;
        }
    }
    if index.is_none() {
        for i in range(0us, MAX_PID_COUNT) {
            if pid_name[i].is_none() {
                index = Some(i);
                pid_name[i] = Some(pid);
                break;
            }
        }
    }
    if index.is_none() {
        panic!("PID array is full");
    }
    pid_cc[index.unwrap()] = Some(cc);
}

fn process_packet(packet: &[u8], pid_name: &mut[Option<u16>], pid_cc: &mut[Option<u16>]) {
    let mut payload;
    let mut pid: u16;
    let mut cc: u16;
    let mut scrambled;
    let mut position = 0;
    let mut last_cc: Option<u16>;
    while position + 187 < 1316 {
        payload = (packet[position + 3] & 16) != 0;
        pid = 256 * (packet[position + 1] as u16 & 0x1f) + packet[position + 2] as u16;
        cc = packet[position + 3] as u16 & 0x0f;
        scrambled = (packet[position + 3] & 192) != 0;
        if packet[position] != 71 {
            continue;
        }
        last_cc = get_pid_cc(pid_name, pid_cc, pid);
        if last_cc.is_some() {
            let lcc = last_cc.unwrap();
            if 16 <= pid && pid <= 8190 && cc != lcc + 1 && (lcc != 15 && cc != 0) && payload {
                println!("CC Error in PID: {}, LastCC: {}, CC: {}", pid, lcc, cc);
            }
            if scrambled {
                println!("Scrambled packet");
            }
        }
        set_pid_cc(pid_name, pid_cc, pid, cc);
        position += 188;
    }
}

fn main() {
    let args = os::args();
    if args.len() != 2 {
        println!("Usage:");
        println!("prober <multicast_group>");
        return;
    }

    let multicast_addr: IpAddr = args[1].as_slice().parse().expect("Invalid value for multicast address!");

    let mut pid_name: [Option<u16>; 8192] = [None; 8192];
    let mut pid_cc: [Option<u16>; 8192] = [None; 8192];
    let mut first_packet_received = false;
    let mut packets_received = 0i32;
    let mut last_stat_time = now().to_timespec();

    let addr = SocketAddr{ ip: Ipv4Addr(0, 0, 0, 0), port: 1234 };

    let mut socket = match UdpSocket::bind(addr) {
        Ok(s) => s,
        Err(e) => panic!("couldn't bind socket: {}", e),
    };

    let join_res = socket.join_multicast(multicast_addr);
    match join_res {
        Err(e) => {
            println!("Join error: {}", e);
            return;
        },
        _ => println!("Joined successfully")
    }

    let mut msg_buff = [0u8; 1316];
    loop {
        let data = socket.recv_from(&mut msg_buff);
        match data {
            Err(e) => {
                println!("Error receiving data: {}", e);
                if first_packet_received {break;}
            },
            Ok((amount, _)) => {
                //println!("Received {} bytes", amount);
                if !first_packet_received {
                    first_packet_received = true;
                    last_stat_time = now().to_timespec();
                }
                process_packet(&msg_buff, &mut pid_name, &mut pid_cc);
                packets_received += 1;

                if packets_received == PACKET_STATISTICS_INTERVAL {
                    let new_time = now().to_timespec();
                    let delta = (new_time - last_stat_time).num_milliseconds() as i32;
                    let pps = PACKET_STATISTICS_INTERVAL / delta;
                    let speed = ((PACKET_STATISTICS_INTERVAL * 1316 / delta) / 1000) * 8;
                    println!("Bitrate: {} Mbps. PPS: {} pps.", speed, pps);
                    last_stat_time = new_time;
                    packets_received = 0;
                }
            }
        }
    }

    drop(socket);
}