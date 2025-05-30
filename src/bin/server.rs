use std::{
    fs, io::{prelude::*, BufReader}, 
    net::{TcpListener, TcpStream}, 
    thread,
    time::Duration
};
use webchathub::ThreadPool;

fn main() {
    let pool = ThreadPool::new(4);

    for _ in 0..4 {

        pool.execute(|| {
            println!("hello world!");
        });
    }
}
