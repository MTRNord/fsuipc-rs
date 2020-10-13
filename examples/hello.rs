//
// FSUIPC library
// Copyright (c) 2015 Alvaro Polo
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

extern crate fsuipc;

use std::io;
use std::process;

use fsuipc::user::*;
use fsuipc::*;

fn main() {
    match run() {
        Ok(_) => process::exit(0),
        Err(e) => {
            println!("IO error: {:?}", e);
            process::exit(-1);
        }
    }
}

fn run() -> io::Result<()> {
    let mut handle = try!(UserHandle::new());
    let mut session = handle.session();
    let mut fsuipc_ver = 0u32;
    let mut fs_ver = 0u16;
    let mut hour = 0u8;
    let mut minute = 0u8;
    let mut second = 0u8;
    try!(session.read(0x3304, &mut fsuipc_ver));
    try!(session.read(0x3308, &mut fs_ver));
    try!(session.read(0x0238, &mut hour));
    try!(session.read(0x0239, &mut minute));
    try!(session.read(0x023a, &mut second));
    try!(session.process());
    println!(
        "FSUIPC version {:x}.{:x}",
        fsuipc_ver >> 28,
        fsuipc_ver >> 20
    );
    println!("FS/P3D version {}", fs_ver);
    println!(
        "Simulation local time is {:02}:{:02}:{:02}",
        hour, minute, second
    );
    Ok(())
}
