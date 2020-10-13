//
// FSUIPC library
// Copyright (c) 2015 Alvaro Polo
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::ffi::CString;
use std::io;
use std::ptr;
use std::sync::{Arc};

use super::ipc::*;
use super::raw::{MutRawBytes, RawBytes};
use super::{Handle, Session};
use winapi::shared::{
    minwindef::{ATOM, LPCVOID},
    windef::HWND,
};
use winapi::um::{
    handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
    memoryapi::{MapViewOfFile, UnmapViewOfFile, FILE_MAP_WRITE},
    processthreadsapi::GetCurrentProcessId,
    winbase::{CreateFileMappingA, GlobalAddAtomA, GlobalDeleteAtom},
    winnt::{HANDLE, PAGE_READWRITE},
    winuser::{FindWindowExA, RegisterWindowMessageA, SendMessageA},
};

#[derive(Clone)]
pub struct UserHandle {
    handle: HWND,
    file_mapping_atom: ATOM,
    file_mapping: HANDLE,
    msg_id: u32,
    data: Arc<*mut u8>,
}

impl UserHandle {
    pub fn new() -> io::Result<Self> {
        unsafe {
            let win_name = CString::new("UIPCMAIN").unwrap();
            let handle = FindWindowExA(
                ptr::null_mut(),
                ptr::null_mut(),
                win_name.as_ptr(),
                ptr::null_mut(),
            );
            if handle.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "cannot connect to user FSUIPC: cannot create window handle",
                ));
            }
            let msg_name = CString::new("FsasmLib:IPC").unwrap();
            let msg_id = RegisterWindowMessageA(msg_name.as_ptr());
            if msg_id == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "cannot connect to user FSUIPC: cannot register window message",
                ));
            }

            let file_mapping_name = CString::new(format!(
                "FsasmLib:IPC:{:x}:{:x}",
                GetCurrentProcessId(),
                next_file_mapping_index()
            ))
            .unwrap();

            let file_mapping_atom = GlobalAddAtomA(file_mapping_name.as_ptr());
            if file_mapping_atom == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "cannot connect to user FSUIPC: cannot add global atom",
                ));
            }

            let file_mapping = CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                ptr::null_mut(),
                PAGE_READWRITE,
                0,
                FILE_MAPPING_LEN as u32,
                file_mapping_name.as_ptr(),
            );
            if file_mapping.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "cannot connect to user FSUIPC: cannot create file mapping",
                ));
            }
            let data = MapViewOfFile(file_mapping, FILE_MAP_WRITE, 0, 0, 0) as *mut u8;
            if data.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "cannot connect to user FSUIPC: cannot map view of file",
                ));
            }
            Ok(UserHandle {
                handle,
                file_mapping_atom,
                file_mapping,
                msg_id,
                data: data.into(),
            })
        }
    }
}

impl Handle for UserHandle {
    type Sess = UserSession;

    fn session(&self) -> UserSession {
        UserSession {
            handle: self.clone(),
            buffer: MutRawBytes::new(self.data.clone(), FILE_MAPPING_LEN),
        }
    }
}

impl Drop for UserHandle {
    fn drop(&mut self) {
        unsafe {
            GlobalDeleteAtom(self.file_mapping_atom);
            UnmapViewOfFile(*self.data as LPCVOID);
            CloseHandle(self.file_mapping);
        }
    }
}

pub struct UserSession {
    handle: UserHandle,
    buffer: MutRawBytes,
}

impl Session for UserSession {
    fn read_bytes(&mut self, offset: u16, dest: *mut u8, len: usize) -> io::Result<usize> {
        self.buffer.write_rsd(offset, dest, len)
    }

    fn write_bytes(&mut self, offset: u16, src: *const u8, len: usize) -> io::Result<usize> {
        self.buffer.write_wsd(offset, src, len)
    }

    fn process(mut self) -> io::Result<usize> {
        unsafe {
            self.buffer.write_header(&MsgHeader::TerminationMark)?;
            let send_result = SendMessageA(
                self.handle.handle,
                self.handle.msg_id,
                self.handle.file_mapping_atom as WinUInt,
                0,
            );
            if send_result != FS6IPC_MESSAGE_SUCCESS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "FSUIPC rejected the requests with error {}; possible buffer corruption",
                        send_result
                    ),
                ));
            }
            let mut buffer = RawBytes::new(*self.handle.data, FILE_MAPPING_LEN);
            loop {
                let header = buffer.read_header()?;
                match header {
                    MsgHeader::ReadStateData {
                        offset: _,
                        len,
                        target,
                    } => {
                        let mut output = MutRawBytes::new(target.into(), len);
                        buffer.read_body(&header, &mut output)?;
                    }
                    MsgHeader::WriteStateData { offset: _, len: _ } => {
                        let mut output = io::sink();
                        buffer.read_body(&header, &mut output)?;
                    }
                    MsgHeader::TerminationMark => return Ok(buffer.consumed()),
                }
            }
        }
    }
}

fn next_file_mapping_index() -> u32 {
    unsafe {
        let next = FILE_MAPPING_INDEX;
        FILE_MAPPING_INDEX += 1;
        next
    }
}

const FS6IPC_MESSAGE_SUCCESS: WinInt = 1;
const FILE_MAPPING_LEN: usize = 64 * 1024;

static mut FILE_MAPPING_INDEX: u32 = 0;
