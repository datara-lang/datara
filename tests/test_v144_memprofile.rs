//! v1.4.4 Track 4: Memory Profiler (`forgen profile --mem`) Tests.

use std::ffi::{CStr, CString};
use std::fs;

#[link(name = "datara_runtime", kind = "static")]
unsafe extern "C" {
    fn datara_memprofile_start();
    fn datara_memprofile_stop();
    fn datara_memprofile_is_active() -> bool;
    fn datara_memprofile_reset();
    fn datara_memprofile_record_alloc(bytes: usize, tag: i32);
    fn datara_memprofile_record_free(bytes: usize, tag: i32);
    fn datara_memprofile_record_promote(bytes: usize);
    fn datara_memprofile_record_slab(hit: bool);
    fn datara_memprofile_summary_json() -> *const std::ffi::c_char;
    fn datara_memprofile_dump(filepath: *const std::ffi::c_char) -> i32;
}

#[test]
fn test_memory_profiler_lifecycle_and_json_report() {
    unsafe {
        datara_memprofile_reset();
        datara_memprofile_start();
        assert!(datara_memprofile_is_active());

        // 1. Record Allocations
        datara_memprofile_record_alloc(1024, 0); // heap
        datara_memprofile_record_alloc(2048, 1); // arena
        datara_memprofile_record_alloc(512, 3); // scratchpad

        // 2. Record Promotions (zero-copy slice promotion into generational arena)
        datara_memprofile_record_promote(4096);

        // 3. Record Slab hits & misses
        datara_memprofile_record_slab(true);
        datara_memprofile_record_slab(true);
        datara_memprofile_record_slab(false);

        // 4. Record Partial Frees
        datara_memprofile_record_free(1024, 0);

        // 5. Inspect JSON Summary
        let c_json = datara_memprofile_summary_json();
        assert!(!c_json.is_null());
        let json_str = CStr::from_ptr(c_json).to_str().expect("valid utf-8");

        let parsed: serde_json::Value = serde_json::from_str(json_str).expect("valid json format");
        assert_eq!(parsed["total_alloc_count"], 3);
        assert_eq!(parsed["total_free_count"], 1);
        assert_eq!(parsed["total_allocated_bytes"], 3584);
        assert_eq!(parsed["total_freed_bytes"], 1024);
        assert_eq!(parsed["current_live_bytes"], 2560);
        assert_eq!(parsed["promotions_count"], 1);
        assert_eq!(parsed["promoted_bytes"], 4096);
        assert_eq!(parsed["slab_hits"], 2);
        assert_eq!(parsed["slab_misses"], 1);

        // 6. Test File Dump
        let out_dir = std::env::temp_dir().join("forgen_test_memprof");
        let _ = fs::create_dir_all(&out_dir);
        let dump_path = out_dir.join("profile_summary.json");
        let c_path = CString::new(dump_path.to_str().unwrap()).unwrap();

        let rc = datara_memprofile_dump(c_path.as_ptr());
        assert_eq!(rc, 0, "dumping memory profile to file must succeed");
        assert!(dump_path.exists());

        let dumped_content = fs::read_to_string(&dump_path).expect("read dumped profile");
        assert!(dumped_content.contains("\"promotions_count\": 1"));

        // 7. Stop and Reset
        datara_memprofile_stop();
        assert!(!datara_memprofile_is_active());

        datara_memprofile_reset();
        let c_json_cleared = datara_memprofile_summary_json();
        let json_cleared = CStr::from_ptr(c_json_cleared)
            .to_str()
            .expect("valid utf-8");
        let parsed_cleared: serde_json::Value =
            serde_json::from_str(json_cleared).expect("valid json");
        assert_eq!(parsed_cleared["total_alloc_count"], 0);

        let _ = fs::remove_file(&dump_path);
    }
}
