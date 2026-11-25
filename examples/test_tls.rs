// use std::{
//     collections::HashSet,
//     sync::atomic::{AtomicPtr, AtomicUsize, Ordering::*},
//     time::{Duration, Instant},
// };

// use rand::RngExt;

// #[cfg(target_pointer_width = "64")]
// const PRIME: usize = 12225810536469756869;

// #[cfg(target_pointer_width = "32")]
// const PRIME: usize = 3256899403;

// const TABLE_SIZE: usize = 2048;

// pub fn thread_id() -> usize {
//     let mut id: usize;
//     unsafe {
//         core::arch::asm!(
//             #[cfg(all(target_arch = "x86_64", unix, not(target_os = "macos")))]
//             "mov {id}, fs:[0]",
//             #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
//             "mov {id}, gs:[0]",
//             #[cfg(all(target_arch = "x86_64", windows))]
//             "mov {id}, gs:[0x30]",
//             #[cfg(all(target_arch = "x86", unix))]
//             "mov {id}, gs:[0]",
//             #[cfg(all(target_arch = "x86", windows))]
//             "mov {id}, fs:[0x18]",
//             // etc for other platforms
//             id = out(reg) id,
//             options(nostack)
//         );
//     }
//     id
// }

// fn thread_hash(id: usize) -> usize {
//     id.wrapping_mul(PRIME) >> (usize::BITS - TABLE_SIZE.ilog2())
// }

// #[repr(C)]
// #[derive(Debug)]
// struct Slot {
//     tid: AtomicUsize,
//     ptr: AtomicPtr<()>,
// }

// static TLS_TABLE: [Slot; TABLE_SIZE] = unsafe { std::mem::zeroed() };

// fn write_tls(ptr: *mut ()) {
//     let mut i = 0;
//     let tid = thread_id();
//     let th = thread_hash(tid);
//     loop {
//         let slot = &TLS_TABLE[th.wrapping_add(i) % TABLE_SIZE];
//         match slot.tid.compare_exchange_weak(0, tid, Relaxed, Relaxed) {
//             Err(c) if c != tid => i = i.wrapping_add(1),
//             _ => {
//                 slot.ptr.store(ptr, Relaxed);
//                 break;
//             }
//         };
//     }
// }

// fn read_tls() -> *mut () {
//     let mut i = 0;
//     let tid = thread_id();
//     let th = thread_hash(tid);
//     loop {
//         let slot = &TLS_TABLE[th.wrapping_add(i) % TABLE_SIZE];
//         if slot.tid.load(Relaxed) == tid {
//             let ptr = slot.ptr.load(Relaxed);
//             slot.tid.store(0, Relaxed);
//             return ptr;
//         }
//         i = i.wrapping_add(1);
//     }
// }

// // let seed = rand::rng().random::<u64>() as usize;

// // println!(
// //     "thread {i} id: {:x} hash: {:x}",
// //     thread_id(),
// //     thread_hash(thread_id())
// // );

// // barrier_ref.wait();

// // let now = Instant::now();
// // for j in 0..1000 {
// //     let value = seed + j;
// //     write_tls(value as *mut ());
// //     assert_eq!(read_tls() as usize, value);
// // }
// // let time = now.elapsed();

// // barrier_ref.wait();

// // println!("thread {i} time: {time:?}");

// pub fn main() {
//     let unique_thread_ids = std::sync::Mutex::new(HashSet::new());
//     let num_active = AtomicUsize::new(0);

//     std::thread::scope(|s| {
//         s.spawn(|| loop {
//             let num = unique_thread_ids.lock().unwrap().len();
//             std::thread::sleep(Duration::from_millis(500));
//             println!(
//                 "unique thread_ids: {num}, active: {}",
//                 num_active.load(Relaxed)
//             )
//         });

//         for _ in 0..65536 {
//             s.spawn(|| {
//                 num_active.fetch_add(1, Relaxed);
//                 unique_thread_ids.lock().unwrap().insert(thread_id());

//                 write_tls(12345 as *mut ());

//                 let sleep_ms = rand::rng().random_range(300..3000);
//                 std::thread::sleep(Duration::from_millis(sleep_ms));
//                 num_active.fetch_sub(1, Relaxed);
//             });

//             std::thread::sleep(Duration::from_millis(1));
//         }

//         println!("all spawned");
//     });
// }

fn main() {}
