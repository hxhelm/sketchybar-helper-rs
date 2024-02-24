use crate::mach::{
    mach_receive_message, read_double_nul_terminated_string_from_address, MachBuffer,
};
use mach2::bootstrap::bootstrap_register;
use mach2::kern_return::KERN_SUCCESS;
use mach2::mach_port::{mach_port_allocate, mach_port_insert_right};
use mach2::message::{mach_msg_destroy, MACH_MSG_TYPE_MAKE_SEND};
use mach2::port::{mach_port_name_t, mach_port_t, MACH_PORT_RIGHT_RECEIVE};
use mach2::task::{task_get_special_port, TASK_BOOTSTRAP_PORT};
use mach2::traps::mach_task_self;
use std::ffi::CString;
use std::os::unix::raw::pthread_t;
use std::ptr::addr_of;
use std::sync::Mutex;

type HandlerT = fn(&str);

struct MachServer {
    is_running: bool,
    task: mach_port_name_t,
    port: mach_port_t,
    bs_port: mach_port_t,
    thread: pthread_t,
    // current handler usage in c implementation: function pointer, defined externally with custom
    // logic and then passed to event_server_begin
    handler: HandlerT,
}

static G_MACH_SERVER: Mutex<MachServer> = Mutex::new(MachServer {
    is_running: false,
    task: 0,
    port: 0,
    bs_port: 0,
    thread: 0,
    handler: |_: &str| Default::default(),
});

fn mach_server_begin(
    mach_server: &mut MachServer,
    handler: HandlerT,
    bootstrap_name: &str,
) -> bool {
    mach_server.task = unsafe { mach_task_self() };

    if unsafe {
        mach_port_allocate(
            mach_server.task,
            MACH_PORT_RIGHT_RECEIVE,
            &mut mach_server.port,
        )
    } != KERN_SUCCESS
    {
        return false;
    }

    if unsafe {
        mach_port_insert_right(
            mach_server.task,
            mach_server.port,
            mach_server.port,
            MACH_MSG_TYPE_MAKE_SEND,
        )
    } != KERN_SUCCESS
    {
        return false;
    }

    if unsafe {
        task_get_special_port(
            mach_server.task,
            TASK_BOOTSTRAP_PORT,
            &mut mach_server.bs_port,
        )
    } != KERN_SUCCESS
    {
        return false;
    }

    if unsafe {
        bootstrap_register(
            mach_server.bs_port,
            CString::new(bootstrap_name)
                .unwrap()
                .as_c_str()
                .as_ptr()
                .cast_mut(),
            mach_server.port,
        )
    } != KERN_SUCCESS
    {
        return false;
    }

    mach_server.handler = handler;
    mach_server.is_running = true;

    let mut buffer = MachBuffer::default();
    while mach_server.is_running {
        mach_receive_message(mach_server.port, &mut buffer, false);
        (mach_server.handler)(unsafe {
            read_double_nul_terminated_string_from_address(
                buffer.message.descriptor.address as *const _,
            )
            .as_str()
        });
        unsafe { mach_msg_destroy(addr_of!(buffer.message.header) as *mut _) };
    }

    true
}

pub fn event_server_begin(event_handler: HandlerT, bootstrap_name: &str) -> bool {
    let mut mach_server = G_MACH_SERVER.lock().unwrap();
    mach_server_begin(&mut mach_server, event_handler, bootstrap_name)
}
