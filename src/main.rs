// Declare externals
extern "C" {
    fn init_360_gadget(await_endpoint_availability: bool) -> i32;
    fn close_360_gadget();
    fn send_to_ep1(fd: i32, data: *const u8, len: usize) -> bool;
    fn gadget_example();
}

fn main() {
    // Call the C function
    println!("Calling init_360_gadget...");
    unsafe { gadget_example() };
}
