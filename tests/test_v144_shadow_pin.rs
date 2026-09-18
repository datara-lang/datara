//! v1.4.4 Track 2: Shadow Heap Pinning for Python Interop Tests.

#[link(name = "datara_runtime", kind = "static")]
unsafe extern "C" {
    fn datara_py_init() -> i32;
    fn datara_py_shadow_pin(var_name: *const i8, ptr: *mut u8, length: usize, elem_size: u32) -> i32;
    fn datara_py_shadow_unpin(ptr: *mut u8);
    fn datara_py_shadow_invalidate_all();
    fn datara_py_shadow_pin_count() -> u32;
}

static SHADOW_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn test_shadow_heap_pinning_lifecycle() {
    let _guard = SHADOW_MUTEX.lock().unwrap();
    unsafe {
        if datara_py_init() != 0 {
            eprintln!("Python runtime not available, skipping test");
            return;
        }

        datara_py_shadow_invalidate_all();
        assert_eq!(datara_py_shadow_pin_count(), 0);

        let mut data = vec![1.0f64, 2.0, 3.0, 4.0, 5.0];
        let var_name = b"features\0".as_ptr() as *const i8;

        // 1. Initial pin
        let rc = datara_py_shadow_pin(var_name, data.as_mut_ptr() as *mut u8, data.len(), 8);
        assert_eq!(rc, 1, "initial shadow pin must succeed");
        assert_eq!(datara_py_shadow_pin_count(), 1);

        // 2. Repeat pin on same pointer: must hit fast path cache (< 500ns)
        let t0 = std::time::Instant::now();
        let rc2 = datara_py_shadow_pin(var_name, data.as_mut_ptr() as *mut u8, data.len(), 8);
        let elapsed = t0.elapsed();
        assert_eq!(rc2, 1, "repeat shadow pin must succeed");
        assert_eq!(datara_py_shadow_pin_count(), 1, "must not duplicate pin slot");
        assert!(elapsed.as_nanos() < 50_000, "repeat pin must be sub-microsecond cache hit: {}ns", elapsed.as_nanos());

        // 3. Unpin
        datara_py_shadow_unpin(data.as_mut_ptr() as *mut u8);
        assert_eq!(datara_py_shadow_pin_count(), 0);

        // 4. Invalidate all clean state
        datara_py_shadow_invalidate_all();
        assert_eq!(datara_py_shadow_pin_count(), 0);
    }
}

#[test]
fn test_shadow_heap_pinning_multiple_buffers() {
    let _guard = SHADOW_MUTEX.lock().unwrap();
    unsafe {
        if datara_py_init() != 0 {
            eprintln!("Python runtime not available, skipping test");
            return;
        }

        datara_py_shadow_invalidate_all();
        assert_eq!(datara_py_shadow_pin_count(), 0);

        let mut buf_a = vec![10i32, 20, 30, 40];
        let mut buf_b = vec![1.5f32, 2.5, 3.5];

        let name_a = b"buf_a\0".as_ptr() as *const i8;
        let name_b = b"buf_b\0".as_ptr() as *const i8;

        let rc1 = datara_py_shadow_pin(name_a, buf_a.as_mut_ptr() as *mut u8, buf_a.len(), 4);
        let rc2 = datara_py_shadow_pin(name_b, buf_b.as_mut_ptr() as *mut u8, buf_b.len(), 4);

        assert_eq!(rc1, 1);
        assert_eq!(rc2, 1);
        assert_eq!(datara_py_shadow_pin_count(), 2);

        // Unpin one buffer
        datara_py_shadow_unpin(buf_a.as_mut_ptr() as *mut u8);
        assert_eq!(datara_py_shadow_pin_count(), 1);

        // Invalidate remaining
        datara_py_shadow_invalidate_all();
        assert_eq!(datara_py_shadow_pin_count(), 0);
    }
}

