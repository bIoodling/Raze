
unsafe fn finish_test_transition() {
    if let Some(a)=UI.get().unwrap().lock().unwrap().animation.as_mut() {
        a.start=Instant::now()-std::time::Duration::from_secs(1);
    }
    tick_animation();
}
unsafe fn run_ui_smoke() {
    // Only the process's own hidden HWNDs. No SendInput, clipboard, cursor or desktop automation.
    transition(Screen::Settings);finish_test_transition();
    assert!(!EDIT.is_null());
    SetWindowTextW(EDIT,wide("950").as_ptr());commit_rate();assert_eq!(snapshot().rate,950);
    SendMessageW(EDIT,EM_SETSEL,0,-1);
    SendMessageW(EDIT,0x00C2,1,wide("1400").as_ptr() as isize); // native EDIT replacement
    assert_eq!(edit_text(),"1400");commit_rate();assert_eq!(snapshot().rate,1400);
    for invalid in ["0","1401","bad",""] {
        SetWindowTextW(EDIT,wide(invalid).as_ptr());commit_rate();
        assert_eq!(snapshot().rate,1400);assert_eq!(edit_text(),"1400");
    }
    UI.get().unwrap().lock().unwrap().capture=CaptureTarget::Macro;
    capture_binding(KeyBinding::mouse(4));
    UI.get().unwrap().lock().unwrap().capture=CaptureTarget::Ui;
    capture_binding(KeyBinding::mouse(4));
    assert!(snapshot().ui_key.is_none());
    capture_binding(KeyBinding::keyboard(0x70));
    assert!(can_activate(snapshot()));
    let saved=fs::read_to_string(data_dir().join("config.ini")).unwrap();
    assert_eq!(encode_config(snapshot()),encode_config(decode_config(&saved)));
    UI.get().unwrap().lock().unwrap().focus=3;
    invoke_focused();assert!(UI.get().unwrap().lock().unwrap().capture==CaptureTarget::Spam);
    capture_binding(KeyBinding::keyboard(65));
    UI.get().unwrap().lock().unwrap().capture=CaptureTarget::Spam;
    capture_binding(KeyBinding::none());assert!(snapshot().spam.is_none());
    transition(Screen::Compact);finish_test_transition();
    assert!(EDIT.is_null());
    transition(Screen::Confirm);finish_test_transition();
    assert!(UI.get().unwrap().lock().unwrap().screen==Screen::Confirm);
    UI.get().unwrap().lock().unwrap().focus=0;invoke_focused();finish_test_transition();
    assert!(UI.get().unwrap().lock().unwrap().screen==Screen::Compact);
    transition(Screen::Activate);finish_test_transition();
    SetWindowTextW(EDIT,wide("wrong").as_ptr());activate();assert!(!license_is_valid());
    SetWindowTextW(EDIT,wide("RAZE-DEV-2026").as_ptr());activate();finish_test_transition();
    assert!(license_is_valid());
    assert!(dev_key_valid(&fs::read_to_string(data_dir().join("development.activation")).unwrap()));
    fs::create_dir_all("dist").unwrap();
    fs::write("dist/ui-smoke.txt",
        "PASS: own hidden HWND lifecycle; native EDIT selection/replacement; rate validation; binding duplicates/clear; disk settings; close/cancel; cached development activation. No physical input generated. Interactive focus, dragging and cadence remain unverified.\n").unwrap();
}
