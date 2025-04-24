fn main() {
    use closure_ffi::BareFnOnce;

    let value = "test".to_owned();
    let bare_closure = BareFnOnce::new_c(move |n: usize| {
        let result = value + &n.to_string();
        println!("{result}");
    });

    // bare() not available on `BareFnOnce` yet
    let bare = bare_closure.leak();

    println!("{:016x}", bare as usize);
    println!("{:02x?}", unsafe {
        &*((bare as usize - 1) as *const [u8; 0x80])
    });


    let result = unsafe { bare(5) };
    //assert_eq!(&result, "test5");
}