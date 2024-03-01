pub mod message;
pub mod server;

use mach2::bootstrap::bootstrap_look_up;
use mach2::kern_return::KERN_SUCCESS;
use mach2::mach_port::{
    mach_port_allocate, mach_port_deallocate, mach_port_insert_right, mach_port_mod_refs,
};
use mach2::message::{
    mach_msg, mach_msg_bits_t, mach_msg_destroy, mach_msg_header_t, mach_msg_id_t,
    mach_msg_ool_descriptor_t, mach_msg_size_t, mach_msg_trailer_t, MACH_MSGH_BITS_COMPLEX,
    MACH_MSGH_BITS_LOCAL_MASK, MACH_MSGH_BITS_PORTS_MASK, MACH_MSGH_BITS_REMOTE_MASK,
    MACH_MSGH_BITS_VOUCHER_MASK, MACH_MSG_SUCCESS, MACH_MSG_TIMEOUT_NONE, MACH_MSG_TYPE_COPY_SEND,
    MACH_MSG_TYPE_MAKE_SEND, MACH_MSG_VIRTUAL_COPY, MACH_RCV_INTERRUPT, MACH_RCV_MSG,
    MACH_RCV_TIMEOUT, MACH_SEND_MSG,
};
use mach2::port::{mach_port_t, MACH_PORT_NULL, MACH_PORT_RIGHT_RECEIVE};
use mach2::task::{task_get_special_port, TASK_BOOTSTRAP_PORT};
use mach2::traps::mach_task_self;
use std::ffi::CString;
use std::mem::size_of;
use std::os::raw::c_char;
use std::ptr::addr_of_mut;
use std::sync::{Mutex, MutexGuard};

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct MachMessage {
    header: mach_msg_header_t,
    msgh_descriptor_count: mach_msg_size_t,
    descriptor: mach_msg_ool_descriptor_t,
}

impl Default for MachMessage {
    fn default() -> Self {
        Self {
            header: mach_msg_header_t::default(),
            msgh_descriptor_count: 0,
            descriptor: mach_msg_ool_descriptor_t::new(
                std::ptr::null_mut(),
                false,
                MACH_MSG_VIRTUAL_COPY,
                0,
            ),
        }
    }
}

#[repr(C, packed)]
struct MachBuffer {
    message: MachMessage,
    trailer: mach_msg_trailer_t,
}

impl Default for MachBuffer {
    fn default() -> Self {
        Self {
            message: MachMessage::default(),
            trailer: mach_msg_trailer_t {
                msgh_trailer_type: 0,
                msgh_trailer_size: 0,
            },
        }
    }
}

impl MachBuffer {
    fn new() -> Self {
        Self::default()
    }

    fn reset(&mut self) {
        self.message = MachMessage::default();
        self.trailer = mach_msg_trailer_t {
            msgh_trailer_type: 0,
            msgh_trailer_size: 0,
        };
    }

    fn receive_message(&mut self, port: mach_port_t, timeout: bool) {
        mach_receive_message(port, self, timeout);
    }

    fn get_response(&self) -> Option<String> {
        if !self.message.descriptor.address.is_null() {
            Some(read_double_nul_terminated_string_from_address(
                self.message.descriptor.address as *const _,
            ))
        } else {
            None
        }
    }
}

static G_MACH_PORT: Mutex<mach_port_t> = Mutex::new(0);

fn get_global_mach_port() -> MutexGuard<'static, mach_port_t> {
    G_MACH_PORT.lock().unwrap()
}

fn mach_receive_message(port: mach_port_t, buffer: &mut MachBuffer, timeout: bool) {
    buffer.reset();

    let header = addr_of_mut!(buffer.message.header);

    let msg_return = match timeout {
        true => unsafe {
            mach_msg(
                header,
                MACH_RCV_MSG | MACH_RCV_TIMEOUT | MACH_RCV_INTERRUPT,
                0,
                size_of::<MachBuffer>() as mach_msg_size_t,
                port,
                1000,
                MACH_PORT_NULL,
            )
        },
        false => unsafe {
            mach_msg(
                header,
                MACH_RCV_MSG,
                0,
                size_of::<MachBuffer>() as mach_msg_size_t,
                port,
                MACH_MSG_TIMEOUT_NONE,
                MACH_PORT_NULL,
            )
        },
    };

    if msg_return != MACH_MSG_SUCCESS {
        buffer.message.descriptor.address = std::ptr::null_mut();
    }
}

fn mach_send_message(port: mach_port_t, message: &mut [u8], length: usize) -> Option<String> {
    if message.is_empty() || port == 0 {
        return None;
    }

    let mut response_port: mach_port_t = 0;
    let task = unsafe { mach_task_self() };

    if unsafe { mach_port_allocate(task, MACH_PORT_RIGHT_RECEIVE, &mut response_port) }
        != KERN_SUCCESS
    {
        return None;
    }

    if unsafe {
        mach_port_insert_right(task, response_port, response_port, MACH_MSG_TYPE_MAKE_SEND)
    } != KERN_SUCCESS
    {
        return None;
    }

    let mut msg = MachMessage::default();

    let mach_msg_size = size_of::<MachMessage>() as mach_msg_size_t;
    msg.header = mach_msg_header_t {
        msgh_bits: MACH_MSGH_BITS_SET(
            MACH_MSG_TYPE_COPY_SEND,
            MACH_MSG_TYPE_MAKE_SEND,
            0,
            MACH_MSGH_BITS_COMPLEX,
        ),
        msgh_size: mach_msg_size,
        msgh_remote_port: port,
        msgh_local_port: response_port,
        msgh_voucher_port: MACH_PORT_NULL,
        msgh_id: response_port as mach_msg_id_t,
    };

    msg.msgh_descriptor_count = 1;

    msg.descriptor = mach_msg_ool_descriptor_t::new(
        message.as_ptr() as *mut _,
        false,
        MACH_MSG_VIRTUAL_COPY,
        (length * size_of::<c_char>()) as u32,
    );

    unsafe {
        mach_msg(
            addr_of_mut!(msg.header),
            MACH_SEND_MSG,
            mach_msg_size,
            0,
            MACH_PORT_NULL,
            MACH_MSG_TIMEOUT_NONE,
            MACH_PORT_NULL,
        )
    };

    let mut buffer = MachBuffer::new();
    buffer.receive_message(response_port, true);
    let response = buffer.get_response();

    unsafe {
        mach_msg_destroy(addr_of_mut!(buffer.message.header));
        mach_port_mod_refs(task, response_port, MACH_PORT_RIGHT_RECEIVE, -1);
        mach_port_deallocate(task, response_port)
    };

    response
}

fn mach_get_bs_port() -> mach_port_t {
    let task = unsafe { mach_task_self() };
    let mut bs_port = 0;

    let kernel_return = unsafe { task_get_special_port(task, TASK_BOOTSTRAP_PORT, &mut bs_port) };

    if kernel_return != KERN_SUCCESS {
        return 0;
    }

    let mut port = 0;

    let service_name = CString::new("git.felix.sketchybar").unwrap();
    let kernel_return = unsafe { bootstrap_look_up(bs_port, service_name.as_ptr(), &mut port) };

    if kernel_return != KERN_SUCCESS {
        return 0;
    }

    port
}

pub fn read_double_nul_terminated_string_from_address(address: *const c_char) -> String {
    let mut caret = 0;
    let mut result = String::new();

    loop {
        if unsafe { *address.add(caret) } == 0 {
            if unsafe { *address.add(caret + 1) } == 0 {
                break;
            }

            result.push('\n');
        } else {
            result.push(char::from(unsafe { *address.add(caret) } as u8));
        }

        caret += 1;
    }

    result
}

#[allow(non_snake_case)]
fn MACH_MSGH_BITS_SET(
    remote: mach_msg_bits_t,
    local: mach_msg_bits_t,
    voucher: mach_msg_bits_t,
    other: mach_msg_bits_t,
) -> mach_msg_bits_t {
    ((remote & MACH_MSGH_BITS_REMOTE_MASK)
        | ((local << 8) & MACH_MSGH_BITS_LOCAL_MASK)
        | ((voucher << 16) & MACH_MSGH_BITS_VOUCHER_MASK))
        | (other & !MACH_MSGH_BITS_PORTS_MASK)
}
