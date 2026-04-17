use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::mpsc;
use std::thread;

fn thread_test(){
    /* share some immutable data across threads */
    let buf0 = Arc::new(vec![1, 2, 3, 4]);
    let a = Arc::clone(&buf0);
    let b = Arc::clone(&buf0);

    let t1 = thread::spawn(move || {
        println!("thread A sees {}", a[0]);
    });

    let t2 = thread::spawn(move || {
        println!("thread B sees {}", b[0]);
    });

    t1.join().unwrap();
    t2.join().unwrap();

    /* shared spsc lock-based buffer */
    /* lock based, so not fit for high throughput & non-blocking work */
    let buf1 = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer_buf = Arc::clone(&buf1);
    let reader_buf = Arc::clone(&buf1);
    
    let writer0 = thread::spawn(move || {
        let mut data = writer_buf.lock().unwrap();
        data.push(42);
        data.push(49);
    });

    let reader0 = thread::spawn(move || {
        let data = reader_buf.lock().unwrap();
        println!("buffer = {:?}", *data);
    });
    writer0.join().unwrap();
    reader0.join().unwrap();
    
    /* shared spmc lock-based buffer */
    /* lock based, so not fit for high throughput & non-blocking work */
    let buf2 = Arc::new(RwLock::new(Vec::<u8>::new()));
    let writer_buf = Arc::clone(&buf2);
    let reader_bf0 = Arc::clone(&buf2);
    let reader_bf1 = Arc::clone(&buf2);

    let writer1 = thread::spawn(move || {
        let mut data = writer_buf.write().unwrap();
        data.push(12);
        data.push(15);
    });

    let reader1 = thread::spawn(move || {
        let data = reader_bf0.read().unwrap();
        println!("buffer = {:?}", *data);
    });

    let reader2 = thread::spawn(move || {
        let data = reader_bf1.read().unwrap();
        println!("buffer = {:?}", *data);
    });
    writer1.join().unwrap();
    reader1.join().unwrap();
    reader2.join().unwrap();

    /* channel across threads */
    /* is this just a domain socket? so slower than shared memory? */
    /* locks on producers? why are we limited to 1 reader? */
    /* what is the underlying algo here? */
    let (tx, rx) =  mpsc::channel();
    let writer2 = thread::spawn(move || {
        for i in 0..5{
            tx.send(i).unwrap();
        }
    });
    let reader3 = thread::spawn(move || {
        while let Ok(value) = rx.recv(){
            println!("got {value}");
        }
    });

    writer2.join().unwrap();
    reader3.join().unwrap();
}
