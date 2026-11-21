// Original timing loop imported unchanged. See docs/TESTING.md.
fn macro_worker(shared: Arc<Shared>) {
    unsafe {
        let mut timer = CreateWaitableTimerExW(null(), null(), 0x0000_0002, TIMER_ALL_ACCESS);
        if timer.is_null() {
            timer = CreateWaitableTimerExW(null(), null(), 0, TIMER_ALL_ACCESS);
        }
        if timer.is_null() {
            return;
        }
        let mut freq = 0i64;
        QueryPerformanceFrequency(&mut freq);
        if freq <= 0 {
            CloseHandle(timer);
            return;
        }
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
        let idle_handles = [shared.stop_event as HANDLE, shared.wake_event as HANDLE];
        let run_handles = [
            shared.stop_event as HANDLE,
            shared.wake_event as HANDLE,
            timer,
        ];
        let guard_ticks = (freq / 10_000).max(1);
        let spin_ticks = (freq / 50_000).max(1);
        'outer: loop {
            if shared.active.load(Ordering::Acquire) && !license_is_valid() {
                shared.active.store(false, Ordering::Release);
            }
            if !shared.active.load(Ordering::Acquire) {
                let r = WaitForMultipleObjects(
                    idle_handles.len() as u32,
                    idle_handles.as_ptr(),
                    0,
                    INFINITE,
                );
                if r == WAIT_OBJECT_0 {
                    break;
                }
                continue;
            }
            let cfg = *shared.config.lock().unwrap();
            let rate = cfg.rate.clamp(1, MAX_RATE) as i64;
            let binding = if cfg.mode == Mode::Cps {
                KeyBinding::mouse(1)
            } else {
                cfg.spam
            };
            let base_ticks = freq / rate;
            let remainder_ticks = freq % rate;
            let mut remainder_acc = 0i64;
            let mut now = 0i64;
            QueryPerformanceCounter(&mut now);
            let mut next_deadline = now;
            let advance = |deadline: &mut i64, rem_acc: &mut i64| {
                *deadline += base_ticks;
                *rem_acc += remainder_ticks;
                if *rem_acc >= rate {
                    *deadline += 1;
                    *rem_acc -= rate;
                }
            };
            advance(&mut next_deadline, &mut remainder_acc);
            loop {
                if !shared.active.load(Ordering::Acquire) || !license_is_valid() {
                    shared.active.store(false, Ordering::Release);
                    CancelWaitableTimer(timer);
                    break;
                }
                QueryPerformanceCounter(&mut now);
                let remain = next_deadline - now;
                if remain > guard_ticks {
                    let wait_ticks = remain - guard_ticks;
                    let hns = (wait_ticks.saturating_mul(10_000_000) / freq).max(1);
                    let due = -hns;
                    if SetWaitableTimer(timer, &due, 0, None, null(), 0) != 0 {
                        let r = WaitForMultipleObjects(
                            run_handles.len() as u32,
                            run_handles.as_ptr(),
                            0,
                            INFINITE,
                        );
                        if r == WAIT_OBJECT_0 {
                            break 'outer;
                        }
                        if r == WAIT_OBJECT_0 + 1 {
                            CancelWaitableTimer(timer);
                            break;
                        }
                    }
                }
                loop {
                    QueryPerformanceCounter(&mut now);
                    let left = next_deadline - now;
                    if left <= 0 {
                        break;
                    }
                    if left > spin_ticks {
                        SwitchToThread();
                    } else {
                        std::hint::spin_loop();
                    }
                }
                if !shared.active.load(Ordering::Acquire) || !license_is_valid() {
                    shared.active.store(false, Ordering::Release);
                    break;
                }
                inject_binding(binding);
                advance(&mut next_deadline, &mut remainder_acc);
                QueryPerformanceCounter(&mut now);
                if next_deadline <= now {
                    next_deadline = now;
                    remainder_acc = 0;
                    advance(&mut next_deadline, &mut remainder_acc);
                }
            }
        }
        CancelWaitableTimer(timer);
        CloseHandle(timer);
    }
}
