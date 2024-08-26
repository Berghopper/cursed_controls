use std::thread::sleep;

// Declare externals
extern "C" {
    fn init_360_gadget(await_endpoint_availability: bool) -> i32;
    fn close_360_gadget(fd: i32);
    fn send_to_ep1(fd: i32, data: *const u8, len: usize) -> bool;
    // fn gadget_example();
}


fn main() {
    // Call the C function
    println!("Calling init_360_gadget...");
    //unsafe { gadget_example() };
    unsafe {
        let fd = init_360_gadget(true);
        while true {
            sleep(std::time::Duration::from_secs(1));
            send_to_ep1(fd, b"\x00\x14\x00\x10\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00".as_ptr(), 20);
            sleep(std::time::Duration::from_secs(1));
            send_to_ep1(fd, b"\x00\x14\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00".as_ptr(), 20);
        }
        
        close_360_gadget(fd);
    }

}