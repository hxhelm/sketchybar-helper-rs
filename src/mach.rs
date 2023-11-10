pub mod message;
mod server;

use mach2::bootstrap::bootstrap_look_up;
use mach2::kern_return::KERN_SUCCESS;
use mach2::mach_port::{mach_port_allocate, mach_port_destroy, mach_port_insert_right};
use mach2::message::{
    mach_msg, mach_msg_bits_t, mach_msg_destroy, mach_msg_header_t, mach_msg_id_t,
    mach_msg_ool_descriptor_t, mach_msg_size_t, mach_msg_trailer_t, MACH_MSGH_BITS_COMPLEX,
    MACH_MSGH_BITS_LOCAL_MASK, MACH_MSGH_BITS_PORTS_MASK, MACH_MSGH_BITS_REMOTE_MASK,
    MACH_MSGH_BITS_VOUCHER_MASK, MACH_MSG_OOL_DESCRIPTOR, MACH_MSG_SUCCESS, MACH_MSG_TIMEOUT_NONE,
    MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MAKE_SEND, MACH_MSG_VIRTUAL_COPY, MACH_RCV_MSG,
    MACH_RCV_TIMEOUT, MACH_SEND_MSG,
};
use mach2::port::{mach_port_t, MACH_PORT_NULL, MACH_PORT_RIGHT_RECEIVE};
use mach2::task::{task_get_special_port, TASK_BOOTSTRAP_PORT};
use mach2::traps::mach_task_self;
use std::ffi::{CStr, CString};
use std::mem::size_of;
use std::os::raw::c_char;
use std::sync::{Mutex, MutexGuard};

#[repr(C)]
#[derive(Copy, Clone)]
struct MachMessage {
    header: mach_msg_header_t,
    msgh_descriptor_count: mach_msg_size_t,
    descriptor: mach_msg_ool_descriptor_t,
}

#[repr(C)]
struct MachBuffer {
    message: MachMessage,
    trailer: mach_msg_trailer_t,
}

impl Default for MachBuffer {
    fn default() -> Self {
        Self {
            message: MachMessage {
                header: mach_msg_header_t {
                    msgh_bits: 0,
                    msgh_size: 0,
                    msgh_remote_port: 0,
                    msgh_local_port: 0,
                    msgh_voucher_port: 0,
                    msgh_id: 0,
                },
                msgh_descriptor_count: 0,
                descriptor: mach_msg_ool_descriptor_t::new(
                    std::ptr::null_mut(),
                    false,
                    MACH_MSG_VIRTUAL_COPY,
                    0,
                ),
            },
            trailer: mach_msg_trailer_t {
                msgh_trailer_type: 0,
                msgh_trailer_size: 0,
            },
        }
    }
}

static G_MACH_PORT: Mutex<mach_port_t> = Mutex::new(0);

fn get_global_mach_port() -> MutexGuard<'static, mach_port_t> {
    G_MACH_PORT.lock().unwrap()
}

fn mach_receive_message(port: mach_port_t, buffer: &mut MachBuffer, timeout: bool) {
    // reset buffer. maybe create a new one instead of passing mutable reference?
    *buffer = MachBuffer::default();

    let msg_return = match timeout {
        true => unsafe {
            mach_msg(
                &mut buffer.message.header,
                MACH_RCV_MSG | MACH_RCV_TIMEOUT,
                0,
                size_of::<MachBuffer>() as mach_msg_size_t,
                port,
                100,
                MACH_PORT_NULL,
            )
        },
        false => unsafe {
            mach_msg(
                &mut buffer.message.header,
                MACH_RCV_MSG,
                0,
                size_of::<MachBuffer>() as mach_msg_size_t,
                port,
                MACH_MSG_TIMEOUT_NONE,
                MACH_PORT_NULL,
            )
        },
    };

    println!(
        "received message: {}, msg_return: {}",
        buffer.message.descriptor.address as u64, msg_return
    );

    if msg_return != MACH_MSG_SUCCESS {
        buffer.message.descriptor.address = std::ptr::null_mut();
    }
}

fn mach_send_message(port: mach_port_t, message: &mut [u8], length: usize) -> Option<CString> {
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

    let mach_msg_size = size_of::<MachMessage>() as mach_msg_size_t;
    let header = mach_msg_header_t {
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

    let msgh_descriptor_count = 1;

    let descriptor = mach_msg_ool_descriptor_t {
        address: message.as_ptr() as *mut _,
        deallocate: 0,
        copy: MACH_MSG_VIRTUAL_COPY as u8,
        pad1: 0,
        type_: MACH_MSG_OOL_DESCRIPTOR as u8,
        size: (length * size_of::<c_char>()) as u32,
    };

    let mut msg = MachMessage {
        header,
        msgh_descriptor_count,
        descriptor,
    };

    let kernel_return = unsafe {
        mach_msg(
            &mut msg.header,
            MACH_SEND_MSG,
            mach_msg_size,
            0,
            MACH_PORT_NULL,
            MACH_MSG_TIMEOUT_NONE,
            MACH_PORT_NULL,
        )
    };

    println!(
        "sent message: {:?}, kernel_return: {}",
        message, kernel_return
    );

    let mut buffer = MachBuffer::default();
    mach_receive_message(response_port, &mut buffer, true);

    if buffer.message.descriptor.address != std::ptr::null_mut() {
        return Some(unsafe {
            let c_str = CStr::from_ptr(buffer.message.descriptor.address as *const _);
            CString::from(c_str)
        });
    }

    unsafe {
        mach_msg_destroy(&mut buffer.message.header);
        mach_port_destroy(task, response_port);
    };

    return None;
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
